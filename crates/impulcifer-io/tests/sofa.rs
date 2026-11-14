#![forbid(unsafe_code)]

use std::path::PathBuf;

use impulcifer_io::{
    SofaMeasurement, SofaMetadata, encode_simple_free_field_hrir, sofa_date,
    write_simple_free_field_hrir,
};
use sofar::reader::{Filter, OpenOptions};

fn metadata() -> SofaMetadata {
    SofaMetadata {
        title: "Impulcifer BRIR".into(),
        comment: "Binaural room impulse responses measured in a room with Impulcifer, stored as SimpleFreeFieldHRIR for player compatibility. Source distance is nominal. Headphone compensation is included.".into(),
        date: "2025-11-14 00:00:00".into(),
    }
}

fn measurements(count: usize, samples: usize) -> Vec<SofaMeasurement> {
    const POSITIONS: [(f64, f64); 7] = [
        (30.0, 0.0),
        (-30.0, 0.0),
        (0.0, 0.0),
        (150.0, 0.0),
        (-150.0, 0.0),
        (90.0, 0.0),
        (-90.0, 0.0),
    ];
    POSITIONS[..count]
        .iter()
        .enumerate()
        .map(|(measurement, &(azimuth, elevation))| SofaMeasurement {
            azimuth,
            elevation,
            distance: 2.0,
            left: (0..samples)
                .map(|sample| measurement as f64 + 0.001 * sample as f64 + 0.1)
                .collect(),
            right: (0..samples)
                .map(|sample| measurement as f64 + 0.001 * sample as f64 + 0.2)
                .collect(),
        })
        .collect()
}

fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "impulcifer-sofa-{name}-{}-{}.sofa",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ))
}

fn open_unchanged(path: &std::path::Path, fs: f32) -> sofar::reader::Sofar {
    OpenOptions::new()
        .normalized(false)
        .sample_rate(fs)
        .open(path)
        .expect("SOFA file should open")
}

#[test]
fn netcdf_c_reference_fixture_opens_in_sofar() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sofa_netcdf_c_reference.sofa");
    let sofa = open_unchanged(&path, 48_000.0);
    assert_eq!(sofa.num_measurements(), 3);
    assert_eq!(sofa.filter_len(), 16);
    assert_eq!(sofa.sample_rate(), 48_000.0);
}

#[test]
fn written_file_opens_in_sofar_with_the_right_shape() {
    let input = measurements(7, 300);
    let path = temp_path("shape");
    write_simple_free_field_hrir(&path, 48_000, &input, &metadata()).unwrap();
    let sofa = open_unchanged(&path, 48_000.0);
    assert_eq!(sofa.num_measurements(), 7);
    assert_eq!(sofa.filter_len(), 300);
    assert_eq!(sofa.sample_rate(), 48_000.0);

    let hrtf = sofa.hrtf();
    assert_eq!(hrtf.source_position.values.len(), 21);
    assert_eq!(hrtf.data_ir.values.len(), 7 * 2 * 300);
    for (measurement, source) in input.iter().enumerate() {
        let azimuth = source.azimuth.to_radians();
        let elevation = source.elevation.to_radians();
        let expected = [
            (source.distance * elevation.cos() * azimuth.cos()) as f32,
            (source.distance * elevation.cos() * azimuth.sin()) as f32,
            (source.distance * elevation.sin()) as f32,
        ];
        let start = measurement * 3;
        for (actual, expected) in hrtf.source_position.values[start..start + 3]
            .iter()
            .zip(expected)
        {
            assert!((actual - expected).abs() < 1e-5);
        }
        let mut filter = Filter::new(300);
        sofa.filter_nointerp(expected[0], expected[1], expected[2], &mut filter);
        for sample in [0, 17, 299] {
            assert!((filter.left[sample] - source.left[sample] as f32).abs() < 1e-6);
            assert!((filter.right[sample] - source.right[sample] as f32).abs() < 1e-6);
        }
    }
    let _ = std::fs::remove_file(path);
}

#[test]
fn written_file_is_deterministic() {
    let input = measurements(7, 300);
    let first = encode_simple_free_field_hrir(48_000, &input, &metadata()).unwrap();
    let second = encode_simple_free_field_hrir(48_000, &input, &metadata()).unwrap();
    assert_eq!(first, second);
}

#[test]
fn measurements_are_padded_and_inputs_validated() {
    let mut input = measurements(2, 4);
    input[0].left.truncate(2);
    input[1].right.truncate(3);
    let path = temp_path("padding");
    write_simple_free_field_hrir(&path, 48_000, &input, &metadata()).unwrap();
    let sofa = open_unchanged(&path, 48_000.0);
    assert_eq!(sofa.filter_len(), 4);
    let data = &sofa.hrtf().data_ir.values;
    assert_eq!(&data[..4], &[0.1, 0.101, 0.0, 0.0]);
    assert_eq!(&data[12..16], &[1.2, 1.201, 1.202, 0.0]);
    assert!(encode_simple_free_field_hrir(48_000, &[], &metadata()).is_err());
    assert!(encode_simple_free_field_hrir(0, &input, &metadata()).is_err());
    input[0].left[0] = f64::NAN;
    assert!(encode_simple_free_field_hrir(48_000, &input, &metadata()).is_err());
    let _ = std::fs::remove_file(path);
}

#[test]
fn sofa_date_formats_utc() {
    assert_eq!(sofa_date(0), "1970-01-01 00:00:00");
    assert_eq!(sofa_date(1_763_078_400), "2025-11-14 00:00:00");
    assert_eq!(sofa_date(1_709_164_800), "2024-02-29 00:00:00");
    assert_eq!(sofa_date(1_709_256_896), "2024-03-01 01:34:56");
}

#[test]
#[ignore]
fn write_sample_files_for_external_readers() {
    let output_dir = std::env::var_os("SOFA_SAMPLE_DIR")
        .map(PathBuf::from)
        .expect("SOFA_SAMPLE_DIR must name the output directory for external-reader samples");
    std::fs::create_dir_all(&output_dir).expect("create SOFA_SAMPLE_DIR");

    const SEVEN_POSITIONS: [(f64, f64); 7] = [
        (30.0, 0.0),
        (-30.0, 0.0),
        (0.0, 0.0),
        (150.0, 0.0),
        (-150.0, 0.0),
        (90.0, 0.0),
        (-90.0, 0.0),
    ];
    let make_measurements = |count: usize, samples: usize, shorten_right: bool| {
        (0..count)
            .map(|measurement| {
                let (azimuth, elevation) = if measurement < SEVEN_POSITIONS.len() {
                    SEVEN_POSITIONS[measurement]
                } else {
                    let extra = measurement - SEVEN_POSITIONS.len();
                    (
                        -165.0 + 30.0 * (extra % 12) as f64,
                        [-45.0, 30.0, 45.0][(extra / 12) % 3],
                    )
                };
                let sample_value = |sample: usize, ear: f64| {
                    (measurement as f64 + 1.0) * 0.03125 + ear + sample as f64 * 0.000_01
                };
                let right_samples = if shorten_right && samples > 1 {
                    samples - 1
                } else {
                    samples
                };
                SofaMeasurement {
                    azimuth,
                    elevation,
                    distance: 2.0,
                    left: (0..samples)
                        .map(|sample| sample_value(sample, 0.125))
                        .collect(),
                    right: (0..right_samples)
                        .map(|sample| sample_value(sample, -0.0625))
                        .collect(),
                }
            })
            .collect::<Vec<_>>()
    };

    for (name, count, samples, shorten_right) in [
        ("small.sofa", 7, 300, true),
        ("long.sofa", 7, 96_000, false),
        ("forty.sofa", 40, 4096, false),
        ("one.sofa", 1, 1, false),
    ] {
        write_simple_free_field_hrir(
            &output_dir.join(name),
            48_000,
            &make_measurements(count, samples, shorten_right),
            &metadata(),
        )
        .unwrap_or_else(|error| panic!("write {name}: {error}"));
    }
}

#[test]
fn written_file_has_no_compact_link_messages() {
    let bytes = encode_simple_free_field_hrir(48_000, &measurements(7, 16), &metadata()).unwrap();
    let root = u64::from_le_bytes(bytes[36..44].try_into().unwrap()) as usize;
    assert_eq!(&bytes[root..root + 4], b"OHDR");
    let mut object_headers = Vec::new();
    let mut search_start = 0;
    while let Some(relative) = bytes[search_start..]
        .windows(4)
        .position(|window| window == b"OHDR")
    {
        object_headers.push(search_start + relative);
        search_start += relative + 4;
    }
    assert!(!object_headers.is_empty());
    let mut root_kinds = Vec::new();
    for header in object_headers {
        let flags = bytes[header + 5];
        let width = 1_usize << (flags & 3);
        let size_start = header + 6 + usize::from(flags & 0x20 != 0) * 16;
        let chunk_size = bytes[size_start..size_start + width]
            .iter()
            .enumerate()
            .fold(0_usize, |value, (index, byte)| {
                value | (usize::from(*byte) << (index * 8))
            });
        let mut cursor = size_start + width;
        let end = cursor + chunk_size;
        let prefix = if flags & 4 != 0 { 6 } else { 4 };
        let mut kinds = Vec::new();
        while cursor + prefix <= end {
            let kind = bytes[cursor];
            let size =
                u16::from_le_bytes(bytes[cursor + 1..cursor + 3].try_into().unwrap()) as usize;
            kinds.push(kind);
            cursor += prefix + size;
        }
        assert!(
            !kinds.contains(&6),
            "compact Link message in object header at {header:#x}: {kinds:?}"
        );
        if header == root {
            root_kinds = kinds;
        }
    }
    assert!(root_kinds.contains(&2), "root has no Link Info message");
}

/// libmysofa 1.3.2 keeps the global heap reader's file position in a 16-bit
/// variable (`src/hdf/gcol.c`), so a collection that ends past 64 KiB loses
/// every `DIMENSION_LIST` reference. The collection must sit before the data.
#[test]
fn global_heap_ends_below_64_kib() {
    for (count, samples) in [(1, 1), (7, 96_000)] {
        let bytes =
            encode_simple_free_field_hrir(48_000, &measurements(count, samples), &metadata())
                .unwrap();
        let collections: Vec<usize> = bytes
            .windows(4)
            .enumerate()
            .filter(|(_, window)| *window == b"GCOL")
            .map(|(index, _)| index)
            .collect();
        assert_eq!(
            collections.len(),
            1,
            "global heap collections: {collections:?}"
        );
        let start = collections[0];
        let size = u64::from_le_bytes(bytes[start + 8..start + 16].try_into().unwrap()) as usize;
        assert!(
            start + size <= 0x1_0000,
            "global heap at {start:#x} of {size} bytes ends past 64 KiB (M = {count}, N = {samples})"
        );
    }
}
