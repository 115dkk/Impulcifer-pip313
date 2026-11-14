//! Deterministic, dependency-free HDF5/netCDF-4 writer for SOFA files.
//!
//! This is intentionally a narrow encoder. It writes exactly the subset needed
//! by `SimpleFreeFieldHRIR`: a version-2 superblock and object headers, dense
//! root links backed by a fractal heap and a version-2 B-tree name index,
//! contiguous little-endian datasets, compact attributes, and netCDF dimension
//! scales with global-heap-backed `DIMENSION_LIST` object references.

use std::{collections::BTreeMap, path::Path};

use crate::IoError;

const UNDEFINED: u64 = u64::MAX;
const SUPERBLOCK_SIZE: usize = 48;
const ROOT_ADDRESS: u64 = SUPERBLOCK_SIZE as u64;
const ROOT_HEADER_CAPACITY: usize = 4096;
const OBJECT_HEADER_CAPACITY: usize = 2048;
const LINK_HEAP_BLOCK_SIZE: usize = 1024;
const BTREE_NODE_SIZE: usize = 512;
const GLOBAL_HEAP_SIZE: usize = 4096;
const NC_DIMENSION_NAME: &str = "This is a netCDF dimension but not a netCDF variable.";

/// One binaural response and its source position.
#[derive(Debug, Clone, PartialEq)]
pub struct SofaMeasurement {
    /// Degrees, SOFA/ITU convention (azimuth positive to the left), distance in metres.
    pub azimuth: f64,
    pub elevation: f64,
    pub distance: f64,
    pub left: Vec<f64>,
    pub right: Vec<f64>,
}

/// File-level values supplied by the application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SofaMetadata {
    pub title: String,
    pub comment: String,
    /// The application/API version string, e.g. "3.21.0".
    pub version: String,
    /// "YYYY-MM-DD HH:MM:SS" (UTC); used for DateCreated and DateModified.
    pub date: String,
}

#[derive(Clone)]
struct Attribute {
    name: String,
    datatype: Vec<u8>,
    shape: Vec<u64>,
    data: Vec<u8>,
    vlen_targets: Option<Vec<u64>>,
}

impl Attribute {
    fn string(name: &str, value: &str) -> Self {
        let mut data = value.as_bytes().to_vec();
        data.push(0);
        Self {
            name: name.into(),
            datatype: datatype_string(data.len()),
            shape: Vec::new(),
            data,
            vlen_targets: None,
        }
    }

    fn i32(name: &str, value: i32) -> Self {
        Self {
            name: name.into(),
            datatype: datatype_i32(),
            shape: Vec::new(),
            data: value.to_le_bytes().to_vec(),
            vlen_targets: None,
        }
    }

    fn vlen_references(
        name: &str,
        targets: &[u64],
        global_heap: u64,
        first_heap_object: u32,
    ) -> Self {
        let mut data = Vec::with_capacity(targets.len() * 16);
        for (index, _) in targets.iter().enumerate() {
            put_u32(&mut data, 1);
            put_u64(&mut data, global_heap);
            put_u32(&mut data, first_heap_object + index as u32);
        }
        Self {
            name: name.into(),
            datatype: datatype_vlen_object_reference(),
            shape: vec![targets.len() as u64],
            data,
            vlen_targets: Some(targets.to_vec()),
        }
    }

    fn reference_list(name: &str, entries: &[(u64, u32)]) -> Self {
        let mut data = Vec::with_capacity(entries.len() * 16);
        for &(dataset, dimension) in entries {
            put_u64(&mut data, dataset);
            put_u32(&mut data, dimension);
            data.extend_from_slice(&[0; 4]);
        }
        Self {
            name: name.into(),
            datatype: datatype_reference_list(),
            shape: vec![entries.len() as u64],
            data,
            vlen_targets: None,
        }
    }
}

type VariableDefinition = (
    &'static str,
    Vec<u64>,
    Vec<f64>,
    &'static [(&'static str, &'static str)],
);

struct Dataset {
    name: String,
    shape: Vec<u64>,
    data: Vec<u8>,
    attributes: Vec<Attribute>,
    header_address: u64,
    data_address: u64,
}

struct Layout {
    root_attributes: Vec<Attribute>,
    datasets: Vec<Dataset>,
    heap_header: u64,
    btree_header: u64,
    btree_leaf: u64,
    heap_block: u64,
    global_heap: u64,
    eof: u64,
}

/// Bytes of a SimpleFreeFieldHRIR 1.0 file (SOFA 2.1).
///
/// Every measurement is padded with zeros to the longest one. Errors are
/// returned for no measurements, a zero sample rate, or any non-finite value.
#[allow(clippy::too_many_lines)]
pub fn encode_simple_free_field_hrir(
    fs: u32,
    measurements: &[SofaMeasurement],
    meta: &SofaMetadata,
) -> Result<Vec<u8>, IoError> {
    validate_input(fs, measurements)?;
    let sample_count = measurements
        .iter()
        .map(|measurement| measurement.left.len().max(measurement.right.len()))
        .max()
        .unwrap_or(0);
    if sample_count == 0 {
        return Err(IoError::InvalidArgument(
            "SOFA measurements must contain at least one sample".into(),
        ));
    }

    let dimensions = [
        ("I", 1_u64),
        ("C", 3),
        ("R", 2),
        ("E", 1),
        ("N", sample_count as u64),
        ("M", measurements.len() as u64),
    ];
    let mut raw_datasets = Vec::new();
    for (index, (name, size)) in dimensions.iter().enumerate() {
        raw_datasets.push(Dataset {
            name: (*name).into(),
            shape: vec![*size],
            data: Vec::new(),
            attributes: vec![
                Attribute::string("CLASS", "DIMENSION_SCALE"),
                Attribute::string("NAME", &format!("{NC_DIMENSION_NAME} {size}")),
                Attribute::i32("_Netcdf4Dimid", index as i32),
            ],
            header_address: 0,
            data_address: UNDEFINED,
        });
    }

    let mut source_positions = Vec::with_capacity(measurements.len() * 3);
    let mut impulse_responses = vec![0.0; measurements.len() * 2 * sample_count];
    for (measurement_index, measurement) in measurements.iter().enumerate() {
        source_positions.extend_from_slice(&[
            measurement.azimuth,
            measurement.elevation,
            measurement.distance,
        ]);
        let left_start = measurement_index * 2 * sample_count;
        let right_start = left_start + sample_count;
        impulse_responses[left_start..left_start + measurement.left.len()]
            .copy_from_slice(&measurement.left);
        impulse_responses[right_start..right_start + measurement.right.len()]
            .copy_from_slice(&measurement.right);
    }

    let variables: [VariableDefinition; 9] = [
        (
            "ListenerPosition",
            vec![1, 3],
            vec![0.0, 0.0, 0.0],
            &[("Type", "cartesian"), ("Units", "metre")],
        ),
        ("ListenerUp", vec![1, 3], vec![0.0, 0.0, 1.0], &[]),
        (
            "ListenerView",
            vec![1, 3],
            vec![1.0, 0.0, 0.0],
            &[("Type", "cartesian"), ("Units", "metre")],
        ),
        (
            "ReceiverPosition",
            vec![2, 3, 1],
            vec![0.0, 0.09, 0.0, 0.0, -0.09, 0.0],
            &[("Type", "cartesian"), ("Units", "metre")],
        ),
        (
            "SourcePosition",
            vec![measurements.len() as u64, 3],
            source_positions,
            &[("Type", "spherical"), ("Units", "degree, degree, metre")],
        ),
        (
            "EmitterPosition",
            vec![1, 3, 1],
            vec![0.0, 0.0, 0.0],
            &[("Type", "cartesian"), ("Units", "metre")],
        ),
        (
            "Data.IR",
            vec![measurements.len() as u64, 2, sample_count as u64],
            impulse_responses,
            &[],
        ),
        (
            "Data.SamplingRate",
            vec![1],
            vec![f64::from(fs)],
            &[("Units", "hertz")],
        ),
        ("Data.Delay", vec![1, 2], vec![0.0, 0.0], &[]),
    ];

    for (name, shape, values, attributes) in variables {
        raw_datasets.push(Dataset {
            name: name.into(),
            shape,
            data: encode_f64(&values),
            attributes: attributes
                .iter()
                .map(|&(name, value)| Attribute::string(name, value))
                .collect(),
            header_address: 0,
            data_address: 0,
        });
    }

    let root_attributes = global_attributes(meta);
    let mut layout = plan_layout(root_attributes, raw_datasets)?;
    add_dimension_attributes(&mut layout)?;
    encode_layout(&layout)
}

/// Write an encoded SOFA file to `path`.
pub fn write_simple_free_field_hrir(
    path: &Path,
    fs: u32,
    measurements: &[SofaMeasurement],
    meta: &SofaMetadata,
) -> Result<(), IoError> {
    std::fs::write(path, encode_simple_free_field_hrir(fs, measurements, meta)?)?;
    Ok(())
}

fn validate_input(fs: u32, measurements: &[SofaMeasurement]) -> Result<(), IoError> {
    if fs == 0 {
        return Err(IoError::InvalidArgument(
            "SOFA sample rate must be greater than zero".into(),
        ));
    }
    if measurements.is_empty() {
        return Err(IoError::InvalidArgument(
            "SOFA requires at least one measurement".into(),
        ));
    }
    for measurement in measurements {
        if !measurement.azimuth.is_finite()
            || !measurement.elevation.is_finite()
            || !measurement.distance.is_finite()
            || measurement.left.iter().any(|value| !value.is_finite())
            || measurement.right.iter().any(|value| !value.is_finite())
        {
            return Err(IoError::InvalidArgument(
                "SOFA values must all be finite".into(),
            ));
        }
    }
    Ok(())
}

fn global_attributes(meta: &SofaMetadata) -> Vec<Attribute> {
    [
        ("Conventions", "SOFA"),
        ("Version", "2.1"),
        ("SOFAConventions", "SimpleFreeFieldHRIR"),
        ("SOFAConventionsVersion", "1.0"),
        ("APIName", "Impulcifer"),
        ("APIVersion", meta.version.as_str()),
        ("ApplicationName", "Impulcifer"),
        ("ApplicationVersion", meta.version.as_str()),
        ("AuthorContact", "unknown"),
        ("Organization", "unknown"),
        (
            "License",
            "No license provided, ask the author for permission",
        ),
        ("DataType", "FIR"),
        ("RoomType", "free field"),
        ("Title", meta.title.as_str()),
        ("DatabaseName", "Impulcifer"),
        ("ListenerShortName", "listener"),
        ("Comment", meta.comment.as_str()),
        ("DateCreated", meta.date.as_str()),
        ("DateModified", meta.date.as_str()),
        ("_NCProperties", "version=2,netcdf=4.10.1,hdf5=2.2.0"),
    ]
    .into_iter()
    .map(|(name, value)| Attribute::string(name, value))
    .collect()
}

fn plan_layout(
    root_attributes: Vec<Attribute>,
    mut datasets: Vec<Dataset>,
) -> Result<Layout, IoError> {
    let mut cursor = align(ROOT_ADDRESS + ROOT_HEADER_CAPACITY as u64, 8);
    for dataset in &mut datasets {
        dataset.header_address = cursor;
        cursor = align(cursor + OBJECT_HEADER_CAPACITY as u64, 8);
    }
    let heap_header = cursor;
    cursor += 146;
    let btree_header = cursor;
    cursor += 38;
    let btree_leaf = cursor;
    cursor += BTREE_NODE_SIZE as u64;
    let heap_block = cursor;
    cursor += LINK_HEAP_BLOCK_SIZE as u64;

    for dataset in &mut datasets {
        if !dataset.data.is_empty() {
            dataset.data_address = cursor;
            cursor = align(cursor + dataset.data.len() as u64, 8);
        }
    }
    let global_heap = cursor;
    cursor += GLOBAL_HEAP_SIZE as u64;

    Ok(Layout {
        root_attributes,
        datasets,
        heap_header,
        btree_header,
        btree_leaf,
        heap_block,
        global_heap,
        eof: cursor,
    })
}

fn add_dimension_attributes(layout: &mut Layout) -> Result<(), IoError> {
    let addresses: BTreeMap<_, _> = layout
        .datasets
        .iter()
        .map(|dataset| (dataset.name.clone(), dataset.header_address))
        .collect();
    let variable_dimensions: [(&str, &[&str]); 9] = [
        ("ListenerPosition", &["I", "C"]),
        ("ListenerUp", &["I", "C"]),
        ("ListenerView", &["I", "C"]),
        ("ReceiverPosition", &["R", "C", "I"]),
        ("SourcePosition", &["M", "C"]),
        ("EmitterPosition", &["E", "C", "I"]),
        ("Data.IR", &["M", "R", "N"]),
        ("Data.SamplingRate", &["I"]),
        ("Data.Delay", &["I", "R"]),
    ];

    let mut back_references: BTreeMap<&str, Vec<(u64, u32)>> = BTreeMap::new();
    let mut next_heap_object = 1_u32;
    for (variable, dimensions) in variable_dimensions {
        let variable_address = addresses[variable];
        let targets = dimensions
            .iter()
            .map(|dimension| addresses[*dimension])
            .collect::<Vec<_>>();
        let dataset = layout
            .datasets
            .iter_mut()
            .find(|dataset| dataset.name == variable)
            .ok_or_else(|| IoError::Format(format!("missing SOFA dataset {variable}")))?;
        dataset.attributes.push(Attribute::vlen_references(
            "DIMENSION_LIST",
            &targets,
            layout.global_heap,
            next_heap_object,
        ));
        next_heap_object += targets.len() as u32;
        for (index, dimension) in dimensions.iter().enumerate() {
            back_references
                .entry(dimension)
                .or_default()
                .push((variable_address, index as u32));
        }
    }
    for (dimension, entries) in back_references {
        let dataset = layout
            .datasets
            .iter_mut()
            .find(|dataset| dataset.name == dimension)
            .ok_or_else(|| IoError::Format(format!("missing SOFA dimension {dimension}")))?;
        dataset
            .attributes
            .push(Attribute::reference_list("REFERENCE_LIST", &entries));
    }
    Ok(())
}

fn encode_layout(layout: &Layout) -> Result<Vec<u8>, IoError> {
    let mut file = vec![0_u8; layout.eof as usize];
    let root_header = object_header(&[
        message(2, link_info(layout)),
        message(10, vec![0, 0]),
        message(21, attribute_info()),
        message(12, attribute_message(&layout.root_attributes[0])?),
        message(
            16,
            continuation_message(ROOT_ADDRESS + ROOT_HEADER_CAPACITY as u64 - 8, 0),
        ),
    ])?;
    let root_attributes = &layout.root_attributes[1..];
    let root_header = root_header_with_continuation(
        &root_header,
        root_attributes,
        ROOT_ADDRESS + ROOT_HEADER_CAPACITY as u64 - 8,
    )?;
    write_at(&mut file, ROOT_ADDRESS, &root_header)?;

    for dataset in &layout.datasets {
        let header = dataset_header(dataset)?;
        write_at(&mut file, dataset.header_address, &header)?;
        if !dataset.data.is_empty() {
            write_at(&mut file, dataset.data_address, &dataset.data)?;
        }
    }

    let links = layout
        .datasets
        .iter()
        .map(|dataset| (dataset.name.as_str(), dataset.header_address))
        .collect::<Vec<_>>();
    write_at(
        &mut file,
        layout.heap_header,
        &fractal_heap_header(layout, &links),
    )?;
    write_at(
        &mut file,
        layout.btree_header,
        &btree_header(layout, links.len()),
    )?;
    write_at(
        &mut file,
        layout.btree_leaf,
        &btree_leaf(&links, layout.heap_header),
    )?;
    write_at(
        &mut file,
        layout.heap_block,
        &fractal_heap_block(&links, layout.heap_header),
    )?;
    write_at(&mut file, layout.global_heap, &global_heap(layout))?;

    // Seal object headers after every address-sensitive structure has been
    // placed. This also makes accidental overlap visible to HDF5 readers as a
    // checksum failure instead of silently accepting stale metadata.
    reseal_object_header(&mut file, ROOT_ADDRESS)?;
    for dataset in &layout.datasets {
        reseal_object_header(&mut file, dataset.header_address)?;
    }
    let superblock = superblock(layout.eof);
    write_at(&mut file, 0, &superblock)?;
    Ok(file)
}

fn dataset_header(dataset: &Dataset) -> Result<Vec<u8>, IoError> {
    let mut messages = vec![
        message(1, dataspace_message(&dataset.shape)),
        message(3, datatype_f64()),
        message(5, vec![3, 10]),
        message(
            8,
            contiguous_layout(dataset.data_address, dataset.data.len() as u64),
        ),
    ];
    for attribute in &dataset.attributes {
        messages.push(message(12, attribute_message(attribute)?));
    }
    object_header(&messages)
}

fn root_header_with_continuation(
    initial: &[u8],
    attributes: &[Attribute],
    continuation_address: u64,
) -> Result<Vec<u8>, IoError> {
    let mut continuation_messages = Vec::new();
    for attribute in attributes {
        continuation_messages.extend_from_slice(&message(12, attribute_message(attribute)?));
    }
    let continuation_length = continuation_messages.len() + 8;
    if continuation_length > ROOT_HEADER_CAPACITY - 256 {
        return Err(IoError::InvalidArgument(
            "SOFA metadata is too large for the deterministic HDF5 header".into(),
        ));
    }
    let mut initial_messages = initial.to_vec();
    let continue_pattern = continuation_message(continuation_address, 0);
    let continuation_offset = ROOT_HEADER_CAPACITY - continuation_length;
    let replacement = continuation_message(
        ROOT_ADDRESS + continuation_offset as u64,
        continuation_length as u64,
    );
    let position = find_bytes(&initial_messages, &continue_pattern).ok_or_else(|| {
        IoError::Format("internal SOFA continuation message was not found".into())
    })?;
    initial_messages[position..position + replacement.len()].copy_from_slice(&replacement);
    let checksum_start = initial_messages.len() - 4;
    let checksum = jenkins_lookup3(&initial_messages[..checksum_start]);
    initial_messages[checksum_start..].copy_from_slice(&checksum.to_le_bytes());

    let mut result = vec![0; ROOT_HEADER_CAPACITY];
    result[..initial_messages.len()].copy_from_slice(&initial_messages);
    let mut continuation = Vec::with_capacity(continuation_length);
    continuation.extend_from_slice(b"OCHK");
    continuation.extend_from_slice(&continuation_messages);
    let checksum = jenkins_lookup3(&continuation);
    put_u32(&mut continuation, checksum);
    result[continuation_offset..].copy_from_slice(&continuation);
    Ok(result)
}

fn superblock(eof: u64) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(SUPERBLOCK_SIZE);
    bytes.extend_from_slice(b"\x89HDF\r\n\x1a\n");
    bytes.extend_from_slice(&[2, 8, 8, 0]);
    put_u64(&mut bytes, 0);
    put_u64(&mut bytes, UNDEFINED);
    put_u64(&mut bytes, eof);
    put_u64(&mut bytes, ROOT_ADDRESS);
    let checksum = jenkins_lookup3(&bytes);
    put_u32(&mut bytes, checksum);
    bytes
}

fn object_header(messages: &[Vec<u8>]) -> Result<Vec<u8>, IoError> {
    let message_len = messages.iter().map(Vec::len).sum::<usize>();
    let (flags, width) = size_width(message_len as u64);
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"OHDR");
    bytes.push(2);
    bytes.push(flags);
    put_var(&mut bytes, message_len as u64, width);
    for message in messages {
        bytes.extend_from_slice(message);
    }
    let checksum = jenkins_lookup3(&bytes);
    put_u32(&mut bytes, checksum);
    Ok(bytes)
}

fn message(kind: u8, payload: Vec<u8>) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(4 + payload.len());
    bytes.push(kind);
    put_u16(&mut bytes, payload.len() as u16);
    bytes.push(0);
    bytes.extend_from_slice(&payload);
    bytes
}

fn continuation_message(address: u64, length: u64) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(16);
    put_u64(&mut bytes, address);
    put_u64(&mut bytes, length);
    bytes
}

fn link_info(layout: &Layout) -> Vec<u8> {
    let mut bytes = vec![0, 0];
    put_u64(&mut bytes, layout.heap_header);
    put_u64(&mut bytes, layout.btree_header);
    bytes
}

fn attribute_info() -> Vec<u8> {
    let mut bytes = vec![0, 0];
    put_u64(&mut bytes, UNDEFINED);
    put_u64(&mut bytes, UNDEFINED);
    bytes
}

fn attribute_message(attribute: &Attribute) -> Result<Vec<u8>, IoError> {
    if attribute.name.len() + 1 > u16::MAX as usize {
        return Err(IoError::InvalidArgument(
            "SOFA HDF5 attribute name is too long".into(),
        ));
    }
    let dataspace = dataspace_message(&attribute.shape);
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&[3, 0]);
    put_u16(&mut bytes, attribute.name.len() as u16 + 1);
    put_u16(&mut bytes, attribute.datatype.len() as u16);
    put_u16(&mut bytes, dataspace.len() as u16);
    bytes.push(0);
    bytes.extend_from_slice(attribute.name.as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(&attribute.datatype);
    bytes.extend_from_slice(&dataspace);
    bytes.extend_from_slice(&attribute.data);
    Ok(bytes)
}

fn dataspace_message(shape: &[u64]) -> Vec<u8> {
    let mut bytes = vec![
        2,
        shape.len() as u8,
        0,
        if shape.is_empty() { 0 } else { 1 },
    ];
    for &dimension in shape {
        put_u64(&mut bytes, dimension);
    }
    bytes
}

fn datatype_f64() -> Vec<u8> {
    let mut bytes = vec![0x11, 0x20, 0x3f, 0];
    put_u32(&mut bytes, 8);
    put_u16(&mut bytes, 0);
    put_u16(&mut bytes, 64);
    bytes.extend_from_slice(&[52, 11, 0, 52]);
    put_u32(&mut bytes, 1023);
    bytes
}

fn datatype_i32() -> Vec<u8> {
    let mut bytes = vec![0x10, 0x08, 0, 0];
    put_u32(&mut bytes, 4);
    put_u16(&mut bytes, 0);
    put_u16(&mut bytes, 32);
    bytes
}

fn datatype_string(length: usize) -> Vec<u8> {
    let mut bytes = vec![0x13, 0, 0, 0];
    put_u32(&mut bytes, length as u32);
    bytes
}

fn datatype_reference() -> Vec<u8> {
    let mut bytes = vec![0x17, 0, 0, 0];
    put_u32(&mut bytes, 8);
    bytes
}

fn datatype_vlen_object_reference() -> Vec<u8> {
    let mut bytes = vec![0x19, 0, 0, 0];
    put_u32(&mut bytes, 16);
    bytes.extend_from_slice(&datatype_reference());
    bytes
}

fn datatype_reference_list() -> Vec<u8> {
    let mut bytes = vec![0x36, 0x02, 0, 0];
    put_u32(&mut bytes, 16);
    bytes.extend_from_slice(b"dataset\0");
    bytes.push(0);
    bytes.extend_from_slice(&datatype_reference());
    bytes.extend_from_slice(b"dimension\0");
    bytes.push(8);
    bytes.extend_from_slice(&datatype_i32());
    bytes
}

fn contiguous_layout(address: u64, length: u64) -> Vec<u8> {
    let mut bytes = vec![3, 1];
    put_u64(&mut bytes, if length == 0 { UNDEFINED } else { address });
    put_u64(&mut bytes, length);
    bytes
}

fn fractal_heap_header(layout: &Layout, links: &[(&str, u64)]) -> Vec<u8> {
    let managed_space = LINK_HEAP_BLOCK_SIZE as u64;
    let used = fractal_link_bytes(links).len() as u64;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"FRHP");
    bytes.push(0);
    put_u16(&mut bytes, 7);
    put_u16(&mut bytes, 0);
    bytes.push(2);
    put_u32(&mut bytes, 4096);
    put_u64(&mut bytes, 0);
    put_u64(&mut bytes, UNDEFINED);
    put_u64(&mut bytes, managed_space - used);
    put_u64(&mut bytes, UNDEFINED);
    put_u64(&mut bytes, managed_space);
    put_u64(&mut bytes, managed_space);
    put_u64(&mut bytes, 0);
    put_u64(&mut bytes, links.len() as u64);
    put_u64(&mut bytes, 0);
    put_u64(&mut bytes, 0);
    put_u64(&mut bytes, 0);
    put_u64(&mut bytes, 0);
    put_u16(&mut bytes, 4);
    put_u64(&mut bytes, LINK_HEAP_BLOCK_SIZE as u64);
    put_u64(&mut bytes, 65_536);
    put_u16(&mut bytes, 32);
    put_u16(&mut bytes, 1);
    put_u64(&mut bytes, layout.heap_block);
    put_u16(&mut bytes, 0);
    let checksum = jenkins_lookup3(&bytes);
    put_u32(&mut bytes, checksum);
    bytes
}

fn fractal_link_bytes(links: &[(&str, u64)]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for (creation_order, &(name, address)) in links.iter().enumerate() {
        bytes.extend_from_slice(&hard_link(name, address, creation_order as u64));
    }
    bytes
}

fn fractal_heap_block(links: &[(&str, u64)], heap_header: u64) -> Vec<u8> {
    let mut bytes = vec![0; LINK_HEAP_BLOCK_SIZE];
    bytes[..4].copy_from_slice(b"FHDB");
    bytes[4] = 0;
    bytes[5..13].copy_from_slice(&heap_header.to_le_bytes());
    bytes[13..17].copy_from_slice(&0_u32.to_le_bytes());
    let links_bytes = fractal_link_bytes(links);
    bytes[21..21 + links_bytes.len()].copy_from_slice(&links_bytes);
    let checksum = jenkins_lookup3(&bytes);
    bytes[17..21].copy_from_slice(&checksum.to_le_bytes());
    bytes
}

fn hard_link(name: &str, address: u64, creation_order: u64) -> Vec<u8> {
    let mut bytes = vec![1, 4];
    put_u64(&mut bytes, creation_order);
    bytes.push(name.len() as u8);
    bytes.extend_from_slice(name.as_bytes());
    put_u64(&mut bytes, address);
    bytes
}

fn btree_header(layout: &Layout, records: usize) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(38);
    bytes.extend_from_slice(b"BTHD");
    bytes.extend_from_slice(&[0, 5]);
    put_u32(&mut bytes, BTREE_NODE_SIZE as u32);
    put_u16(&mut bytes, 11);
    put_u16(&mut bytes, 0);
    bytes.extend_from_slice(&[100, 40]);
    put_u64(&mut bytes, layout.btree_leaf);
    put_u16(&mut bytes, records as u16);
    put_u64(&mut bytes, records as u64);
    let checksum = jenkins_lookup3(&bytes);
    put_u32(&mut bytes, checksum);
    bytes
}

fn btree_leaf(links: &[(&str, u64)], heap_header: u64) -> Vec<u8> {
    let mut offsets = Vec::with_capacity(links.len());
    let mut offset = 21_u32;
    for &(name, _) in links {
        let length = hard_link(name, 0, 0).len() as u16;
        offsets.push((name, offset, length));
        offset += u32::from(length);
    }
    offsets.sort_by_key(|(name, _, _)| jenkins_lookup3(name.as_bytes()));
    let mut bytes = Vec::with_capacity(BTREE_NODE_SIZE);
    bytes.extend_from_slice(b"BTLF");
    bytes.push(0);
    bytes.push(5);
    for (name, offset, length) in offsets {
        put_u32(&mut bytes, jenkins_lookup3(name.as_bytes()));
        bytes.extend_from_slice(&heap_id(offset, length, heap_header));
    }
    let checksum = jenkins_lookup3(&bytes);
    put_u32(&mut bytes, checksum);
    bytes.resize(BTREE_NODE_SIZE, 0);
    bytes
}

fn heap_id(offset: u32, length: u16, _heap_header: u64) -> [u8; 7] {
    // Managed-object ID: the first byte is the managed-object type (zero),
    // followed by a 4-byte heap offset and a 2-byte length.
    let mut result = [0_u8; 7];
    result[1..5].copy_from_slice(&offset.to_le_bytes());
    result[5..7].copy_from_slice(&length.to_le_bytes());
    result
}

fn global_heap(layout: &Layout) -> Vec<u8> {
    let sequences = layout
        .datasets
        .iter()
        .flat_map(|dataset| &dataset.attributes)
        .filter_map(|attribute| attribute.vlen_targets.as_ref())
        .flatten()
        .copied()
        .collect::<Vec<_>>();
    let mut bytes = Vec::with_capacity(GLOBAL_HEAP_SIZE);
    bytes.extend_from_slice(b"GCOL");
    bytes.extend_from_slice(&[1, 0, 0, 0]);
    put_u64(&mut bytes, GLOBAL_HEAP_SIZE as u64);
    for (index, &address) in sequences.iter().enumerate() {
        put_u16(&mut bytes, index as u16 + 1);
        put_u16(&mut bytes, 0);
        put_u32(&mut bytes, 0);
        put_u64(&mut bytes, 8);
        put_u64(&mut bytes, address);
    }
    let remaining = GLOBAL_HEAP_SIZE - bytes.len();
    put_u16(&mut bytes, 0);
    put_u16(&mut bytes, 0);
    put_u32(&mut bytes, 0);
    put_u64(&mut bytes, remaining as u64);
    bytes.resize(GLOBAL_HEAP_SIZE, 0);
    bytes
}

fn encode_f64(values: &[f64]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * 8);
    for &value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn reseal_object_header(file: &mut [u8], address: u64) -> Result<(), IoError> {
    let start = address as usize;
    if file.get(start..start + 4) != Some(b"OHDR") {
        return Err(IoError::Format(
            "SOFA object header signature is missing".into(),
        ));
    }
    let flags = file[start + 5];
    let width = 1_usize << (flags & 3);
    let size_start = start + 6 + usize::from(flags & 0x20 != 0) * 16;
    let chunk_size = file[size_start..size_start + width]
        .iter()
        .enumerate()
        .fold(0_usize, |value, (index, byte)| {
            value | (usize::from(*byte) << (8 * index))
        });
    let checksum_start = size_start + width + chunk_size;
    let checksum = jenkins_lookup3(&file[start..checksum_start]);
    file[checksum_start..checksum_start + 4].copy_from_slice(&checksum.to_le_bytes());
    Ok(())
}

fn write_at(file: &mut [u8], address: u64, value: &[u8]) -> Result<(), IoError> {
    let start = address as usize;
    let end = start
        .checked_add(value.len())
        .ok_or_else(|| IoError::Format("SOFA HDF5 address overflowed the platform size".into()))?;
    if end > file.len() {
        return Err(IoError::Format(
            "SOFA HDF5 structure exceeds planned file size".into(),
        ));
    }
    file[start..end].copy_from_slice(value);
    Ok(())
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn align(value: u64, boundary: u64) -> u64 {
    (value + boundary - 1) & !(boundary - 1)
}

fn size_width(value: u64) -> (u8, usize) {
    if value <= u8::MAX as u64 {
        (0, 1)
    } else if value <= u16::MAX as u64 {
        (1, 2)
    } else if value <= u32::MAX as u64 {
        (2, 4)
    } else {
        (3, 8)
    }
}

fn put_var(bytes: &mut Vec<u8>, value: u64, width: usize) {
    bytes.extend_from_slice(&value.to_le_bytes()[..width]);
}

fn put_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn put_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn jenkins_lookup3(data: &[u8]) -> u32 {
    fn rotate(value: u32, shift: u32) -> u32 {
        value.rotate_left(shift)
    }
    fn mix(mut a: u32, mut b: u32, mut c: u32) -> (u32, u32, u32) {
        a = a.wrapping_sub(c);
        a ^= rotate(c, 4);
        c = c.wrapping_add(b);
        b = b.wrapping_sub(a);
        b ^= rotate(a, 6);
        a = a.wrapping_add(c);
        c = c.wrapping_sub(b);
        c ^= rotate(b, 8);
        b = b.wrapping_add(a);
        a = a.wrapping_sub(c);
        a ^= rotate(c, 16);
        c = c.wrapping_add(b);
        b = b.wrapping_sub(a);
        b ^= rotate(a, 19);
        a = a.wrapping_add(c);
        c = c.wrapping_sub(b);
        c ^= rotate(b, 4);
        b = b.wrapping_add(a);
        (a, b, c)
    }
    fn final_mix(mut a: u32, mut b: u32, mut c: u32) -> u32 {
        c ^= b;
        c = c.wrapping_sub(rotate(b, 14));
        a ^= c;
        a = a.wrapping_sub(rotate(c, 11));
        b ^= a;
        b = b.wrapping_sub(rotate(a, 25));
        c ^= b;
        c = c.wrapping_sub(rotate(b, 16));
        a ^= c;
        a = a.wrapping_sub(rotate(c, 4));
        b ^= a;
        b = b.wrapping_sub(rotate(a, 14));
        c ^= b;
        c.wrapping_sub(rotate(b, 24))
    }

    let initial = 0xdead_beef_u32.wrapping_add(data.len() as u32);
    let (mut a, mut b, mut c) = (initial, initial, initial);
    let mut consumed = 0;
    while data.len() - consumed > 12 {
        let chunk = &data[consumed..consumed + 12];
        a = a.wrapping_add(u32::from_le_bytes(chunk[0..4].try_into().unwrap()));
        b = b.wrapping_add(u32::from_le_bytes(chunk[4..8].try_into().unwrap()));
        c = c.wrapping_add(u32::from_le_bytes(chunk[8..12].try_into().unwrap()));
        (a, b, c) = mix(a, b, c);
        consumed += 12;
    }
    let remainder = &data[consumed..];
    if remainder.is_empty() {
        return c;
    }
    for (index, &value) in remainder.iter().enumerate() {
        match index {
            0..=3 => a = a.wrapping_add(u32::from(value) << (index * 8)),
            4..=7 => b = b.wrapping_add(u32::from(value) << ((index - 4) * 8)),
            _ => c = c.wrapping_add(u32::from(value) << ((index - 8) * 8)),
        }
    }
    final_mix(a, b, c)
}

/// Format Unix seconds as UTC using a proleptic Gregorian calendar.
pub fn sofa_date(unix_seconds: u64) -> String {
    let days = unix_seconds / 86_400;
    let seconds = unix_seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = seconds / 3_600;
    let minute = seconds / 60 % 60;
    let second = seconds % 60;
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
}

fn civil_from_days(days_since_epoch: u64) -> (u64, u64, u64) {
    // Howard Hinnant's civil-from-days transform, adapted to unsigned Unix time.
    let z = days_since_epoch + 719_468;
    let era = z / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    year += u64::from(month <= 2);
    (year, month, day)
}
