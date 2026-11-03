use std::collections::BTreeMap;

use super::ExternalError;

const BASE_WIRE_HANDLE: u32 = 0x7e0000;
const MAX_DEPTH: usize = 64;
const MAX_HANDLES: usize = 4_000_000;

const TC_NULL: u8 = 0x70;
const TC_REFERENCE: u8 = 0x71;
const TC_CLASSDESC: u8 = 0x72;
const TC_OBJECT: u8 = 0x73;
const TC_STRING: u8 = 0x74;
const TC_ARRAY: u8 = 0x75;
const TC_CLASS: u8 = 0x76;
const TC_BLOCKDATA: u8 = 0x77;
const TC_ENDBLOCKDATA: u8 = 0x78;
const TC_RESET: u8 = 0x79;
const TC_BLOCKDATALONG: u8 = 0x7a;
const TC_EXCEPTION: u8 = 0x7b;
const TC_LONGSTRING: u8 = 0x7c;
const TC_PROXYCLASSDESC: u8 = 0x7d;
const TC_ENUM: u8 = 0x7e;

const SC_WRITE_METHOD: u8 = 0x01;
const SC_SERIALIZABLE: u8 = 0x02;
const SC_EXTERNALIZABLE: u8 = 0x04;
const SC_BLOCK_DATA: u8 = 0x08;

pub(crate) type ValueRef = usize;

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub(crate) enum FieldValue {
    Byte(i8),
    Char(u16),
    Double(f64),
    Float(f32),
    Int(i32),
    Long(i64),
    Short(i16),
    Bool(bool),
    Value(Option<ValueRef>),
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub(crate) struct ClassFields {
    pub(crate) class_name: String,
    pub(crate) fields: BTreeMap<String, FieldValue>,
}

#[derive(Clone, Debug)]
pub(crate) struct ObjectValue {
    pub(crate) class_name: String,
    pub(crate) classes: Vec<ClassFields>,
}

impl ObjectValue {
    pub(crate) fn field(&self, name: &str) -> Option<&FieldValue> {
        self.classes.iter().find_map(|class| class.fields.get(name))
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub(crate) enum Value {
    String(String),
    Object(ObjectValue),
    Bytes(Vec<i8>),
    Chars(Vec<u16>),
    Doubles(Vec<f64>),
    Floats(Vec<f32>),
    Ints(Vec<i32>),
    Longs(Vec<i64>),
    Shorts(Vec<i16>),
    Bools(Vec<bool>),
    Objects(Vec<Option<ValueRef>>),
    Enum { class_name: String, name: ValueRef },
    Class(String),
    ClassDesc(ClassDesc),
    Pending,
}

#[derive(Clone, Debug)]
struct FieldDesc {
    code: u8,
    name: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ClassDesc {
    name: String,
    flags: u8,
    fields: Vec<FieldDesc>,
    parent: Option<ValueRef>,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub(crate) enum TopContent {
    Value(Option<ValueRef>),
    BlockData(Vec<u8>),
}

#[derive(Debug)]
pub(crate) struct JavaStream {
    pub(crate) values: Vec<Value>,
    pub(crate) contents: Vec<TopContent>,
}

impl JavaStream {
    pub(crate) fn value(&self, reference: ValueRef) -> Result<&Value, ExternalError> {
        self.values
            .get(reference)
            .ok_or_else(|| ExternalError::new("invalid Java stream reference"))
    }
}

pub(crate) fn read(bytes: &[u8]) -> Result<JavaStream, ExternalError> {
    Parser::new(bytes).parse()
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
    values: Vec<Value>,
    handles: Vec<ValueRef>,
}

impl<'a> Parser<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            pos: 0,
            values: Vec::new(),
            handles: Vec::new(),
        }
    }

    fn parse(mut self) -> Result<JavaStream, ExternalError> {
        if self.u16()? != 0xaced || self.u16()? != 5 {
            return Err(ExternalError::new("not a Java serialization stream"));
        }
        let mut contents = Vec::new();
        while self.pos < self.bytes.len() {
            let tag = self.u8()?;
            match tag {
                TC_BLOCKDATA => {
                    let n = usize::from(self.u8()?);
                    contents.push(TopContent::BlockData(self.take(n)?.to_vec()));
                }
                TC_BLOCKDATALONG => {
                    let n = self.length_u32()?;
                    contents.push(TopContent::BlockData(self.take(n)?.to_vec()));
                }
                TC_RESET => self.handles.clear(),
                TC_ENDBLOCKDATA => {
                    return Err(ExternalError::new("unexpected end block data"));
                }
                _ => contents.push(TopContent::Value(self.content_from_tag(tag, 0)?)),
            }
        }
        Ok(JavaStream {
            values: self.values,
            contents,
        })
    }

    fn content(&mut self, depth: usize) -> Result<Option<ValueRef>, ExternalError> {
        self.check_depth(depth)?;
        let tag = self.u8()?;
        self.content_from_tag(tag, depth)
    }

    fn content_from_tag(
        &mut self,
        tag: u8,
        depth: usize,
    ) -> Result<Option<ValueRef>, ExternalError> {
        self.check_depth(depth)?;
        match tag {
            TC_NULL => Ok(None),
            TC_REFERENCE => self.reference().map(Some),
            TC_STRING => self.new_string(false).map(Some),
            TC_LONGSTRING => self.new_string(true).map(Some),
            TC_CLASSDESC => self.new_class_desc(depth + 1).map(Some),
            TC_OBJECT => self.new_object(depth + 1).map(Some),
            TC_ARRAY => self.new_array(depth + 1).map(Some),
            TC_CLASS => self.new_class(depth + 1).map(Some),
            TC_ENUM => self.new_enum(depth + 1).map(Some),
            TC_RESET => {
                self.handles.clear();
                self.content(depth + 1)
            }
            TC_EXCEPTION => Err(ExternalError::new("Java exception content is unsupported")),
            TC_PROXYCLASSDESC => Err(ExternalError::new(
                "Java proxy class descriptions are unsupported",
            )),
            _ => Err(ExternalError::new(format!(
                "invalid Java stream type 0x{tag:02x}"
            ))),
        }
    }

    fn new_string(&mut self, long: bool) -> Result<ValueRef, ExternalError> {
        let n = if long {
            self.length_u64()?
        } else {
            usize::from(self.u16()?)
        };
        let raw = self.take(n)?;
        let value = Value::String(decode_modified_utf8(raw));
        self.add_handle(value)
    }

    fn new_class_desc(&mut self, depth: usize) -> Result<ValueRef, ExternalError> {
        self.check_depth(depth)?;
        let name = self.utf()?;
        let _serial_uid = self.u64()?;
        let reference = self.add_handle(Value::Pending)?;
        let flags = self.u8()?;
        let field_count = usize::from(self.u16()?);
        // Every field needs at least a type byte and an empty UTF name.
        if field_count > self.remaining() / 3 {
            return Err(ExternalError::new(
                "Java field count exceeds remaining bytes",
            ));
        }
        let mut fields = Vec::with_capacity(field_count);
        for _ in 0..field_count {
            let code = self.u8()?;
            if !matches!(
                code,
                b'B' | b'C' | b'D' | b'F' | b'I' | b'J' | b'S' | b'Z' | b'L' | b'['
            ) {
                return Err(ExternalError::new("invalid Java field type"));
            }
            let field_name = self.utf()?;
            if matches!(code, b'L' | b'[') {
                let type_ref = self.content(depth + 1)?;
                match type_ref.and_then(|r| self.values.get(r)) {
                    Some(Value::String(_)) => {}
                    _ => return Err(ExternalError::new("invalid Java field class name")),
                }
            }
            fields.push(FieldDesc {
                code,
                name: field_name,
            });
        }
        self.skip_annotations(depth + 1)?;
        let parent = self.class_desc(depth + 1)?;
        self.values[reference] = Value::ClassDesc(ClassDesc {
            name,
            flags,
            fields,
            parent,
        });
        Ok(reference)
    }

    fn class_desc(&mut self, depth: usize) -> Result<Option<ValueRef>, ExternalError> {
        self.check_depth(depth)?;
        let tag = self.u8()?;
        match tag {
            TC_NULL => Ok(None),
            TC_REFERENCE => {
                let r = self.reference()?;
                if !matches!(self.values.get(r), Some(Value::ClassDesc(_))) {
                    return Err(ExternalError::new("reference is not a class description"));
                }
                Ok(Some(r))
            }
            TC_CLASSDESC => self.new_class_desc(depth + 1).map(Some),
            TC_PROXYCLASSDESC => Err(ExternalError::new(
                "Java proxy class descriptions are unsupported",
            )),
            _ => Err(ExternalError::new("invalid Java class description")),
        }
    }

    fn new_object(&mut self, depth: usize) -> Result<ValueRef, ExternalError> {
        let class_ref = self
            .class_desc(depth + 1)?
            .ok_or_else(|| ExternalError::new("object has no class description"))?;
        let object_ref = self.add_handle(Value::Pending)?;
        let hierarchy = self.hierarchy(class_ref)?;
        let class_name = hierarchy
            .last()
            .map(|d| d.name.clone())
            .ok_or_else(|| ExternalError::new("empty Java class hierarchy"))?;
        let mut classes = Vec::with_capacity(hierarchy.len());
        for desc in hierarchy {
            let mut fields = BTreeMap::new();
            if desc.flags & SC_SERIALIZABLE != 0 {
                for field in &desc.fields {
                    fields.insert(field.name.clone(), self.field_value(field.code, depth + 1)?);
                }
                if desc.flags & SC_WRITE_METHOD != 0 {
                    self.skip_annotations(depth + 1)?;
                }
            } else if desc.flags & SC_EXTERNALIZABLE != 0 {
                if desc.flags & SC_BLOCK_DATA == 0 {
                    return Err(ExternalError::new(
                        "unblocked externalizable Java class is unsupported",
                    ));
                }
                self.skip_annotations(depth + 1)?;
            } else {
                return Err(ExternalError::new("unsupported Java class flags"));
            }
            classes.push(ClassFields {
                class_name: desc.name,
                fields,
            });
        }
        self.values[object_ref] = Value::Object(ObjectValue {
            class_name,
            classes,
        });
        Ok(object_ref)
    }

    fn hierarchy(&self, mut class_ref: ValueRef) -> Result<Vec<ClassDesc>, ExternalError> {
        let mut reverse = Vec::new();
        loop {
            let desc = match self.values.get(class_ref) {
                Some(Value::ClassDesc(desc)) => desc,
                _ => return Err(ExternalError::new("invalid Java class hierarchy")),
            };
            reverse.push(desc.clone());
            if reverse.len() > MAX_DEPTH {
                return Err(ExternalError::new("Java nesting limit exceeded"));
            }
            match desc.parent {
                Some(parent) => class_ref = parent,
                None => break,
            }
        }
        reverse.reverse();
        Ok(reverse)
    }

    fn field_value(&mut self, code: u8, depth: usize) -> Result<FieldValue, ExternalError> {
        Ok(match code {
            b'B' => FieldValue::Byte(self.u8()? as i8),
            b'C' => FieldValue::Char(self.u16()?),
            b'D' => FieldValue::Double(f64::from_bits(self.u64()?)),
            b'F' => FieldValue::Float(f32::from_bits(self.u32()?)),
            b'I' => FieldValue::Int(self.u32()? as i32),
            b'J' => FieldValue::Long(self.u64()? as i64),
            b'S' => FieldValue::Short(self.u16()? as i16),
            b'Z' => FieldValue::Bool(self.u8()? != 0),
            b'L' | b'[' => FieldValue::Value(self.content(depth + 1)?),
            _ => return Err(ExternalError::new("invalid Java field type")),
        })
    }

    fn new_array(&mut self, depth: usize) -> Result<ValueRef, ExternalError> {
        let class_ref = self
            .class_desc(depth + 1)?
            .ok_or_else(|| ExternalError::new("array has no class description"))?;
        let class_name = match self.values.get(class_ref) {
            Some(Value::ClassDesc(desc)) => desc.name.clone(),
            _ => return Err(ExternalError::new("invalid array class description")),
        };
        let array_ref = self.add_handle(Value::Pending)?;
        let n = self.length_i32()?;
        let code = class_name
            .as_bytes()
            .get(1)
            .copied()
            .ok_or_else(|| ExternalError::new("invalid Java array class name"))?;
        let element_size = match code {
            b'B' | b'Z' => 1,
            b'C' | b'S' => 2,
            b'I' | b'F' => 4,
            b'J' | b'D' => 8,
            b'L' | b'[' => 1,
            _ => return Err(ExternalError::new("invalid Java array type")),
        };
        if n.checked_mul(element_size)
            .is_none_or(|minimum| minimum > self.remaining())
        {
            return Err(ExternalError::new("Java array exceeds remaining bytes"));
        }
        let value = match code {
            b'B' => Value::Bytes(
                (0..n)
                    .map(|_| self.u8().map(|v| v as i8))
                    .collect::<Result<_, _>>()?,
            ),
            b'Z' => Value::Bools(
                (0..n)
                    .map(|_| self.u8().map(|v| v != 0))
                    .collect::<Result<_, _>>()?,
            ),
            b'C' => Value::Chars((0..n).map(|_| self.u16()).collect::<Result<_, _>>()?),
            b'S' => Value::Shorts(
                (0..n)
                    .map(|_| self.u16().map(|v| v as i16))
                    .collect::<Result<_, _>>()?,
            ),
            b'I' => Value::Ints(
                (0..n)
                    .map(|_| self.u32().map(|v| v as i32))
                    .collect::<Result<_, _>>()?,
            ),
            b'F' => Value::Floats(
                (0..n)
                    .map(|_| self.u32().map(f32::from_bits))
                    .collect::<Result<_, _>>()?,
            ),
            b'J' => Value::Longs(
                (0..n)
                    .map(|_| self.u64().map(|v| v as i64))
                    .collect::<Result<_, _>>()?,
            ),
            b'D' => Value::Doubles(
                (0..n)
                    .map(|_| self.u64().map(f64::from_bits))
                    .collect::<Result<_, _>>()?,
            ),
            b'L' | b'[' => Value::Objects(
                (0..n)
                    .map(|_| self.content(depth + 1))
                    .collect::<Result<_, _>>()?,
            ),
            _ => unreachable!(),
        };
        self.values[array_ref] = value;
        Ok(array_ref)
    }

    fn new_class(&mut self, depth: usize) -> Result<ValueRef, ExternalError> {
        let desc = self
            .class_desc(depth + 1)?
            .ok_or_else(|| ExternalError::new("class has no description"))?;
        let name = match self.values.get(desc) {
            Some(Value::ClassDesc(desc)) => desc.name.clone(),
            _ => return Err(ExternalError::new("invalid class description")),
        };
        self.add_handle(Value::Class(name))
    }

    fn new_enum(&mut self, depth: usize) -> Result<ValueRef, ExternalError> {
        let desc_ref = self
            .class_desc(depth + 1)?
            .ok_or_else(|| ExternalError::new("enum has no class description"))?;
        let enum_ref = self.add_handle(Value::Pending)?;
        let name = self
            .content(depth + 1)?
            .ok_or_else(|| ExternalError::new("enum has null name"))?;
        if !matches!(self.values.get(name), Some(Value::String(_))) {
            return Err(ExternalError::new("enum name is not a string"));
        }
        let class_name = match self.values.get(desc_ref) {
            Some(Value::ClassDesc(desc)) => desc.name.clone(),
            _ => return Err(ExternalError::new("invalid enum class description")),
        };
        self.values[enum_ref] = Value::Enum { class_name, name };
        Ok(enum_ref)
    }

    fn skip_annotations(&mut self, depth: usize) -> Result<(), ExternalError> {
        self.check_depth(depth)?;
        loop {
            let tag = self.u8()?;
            match tag {
                TC_ENDBLOCKDATA => return Ok(()),
                TC_BLOCKDATA => {
                    let n = usize::from(self.u8()?);
                    self.take(n)?;
                }
                TC_BLOCKDATALONG => {
                    let n = self.length_u32()?;
                    self.take(n)?;
                }
                TC_RESET => self.handles.clear(),
                _ => {
                    self.content_from_tag(tag, depth + 1)?;
                }
            }
        }
    }

    fn add_handle(&mut self, value: Value) -> Result<ValueRef, ExternalError> {
        if self.handles.len() >= MAX_HANDLES {
            return Err(ExternalError::new("Java handle limit exceeded"));
        }
        let reference = self.values.len();
        self.values.push(value);
        self.handles.push(reference);
        Ok(reference)
    }

    fn reference(&mut self) -> Result<ValueRef, ExternalError> {
        let wire = self.u32()?;
        let index = wire
            .checked_sub(BASE_WIRE_HANDLE)
            .and_then(|v| usize::try_from(v).ok())
            .ok_or_else(|| ExternalError::new("invalid Java stream reference"))?;
        self.handles
            .get(index)
            .copied()
            .ok_or_else(|| ExternalError::new("invalid Java stream reference"))
    }

    fn utf(&mut self) -> Result<String, ExternalError> {
        let n = usize::from(self.u16()?);
        Ok(decode_modified_utf8(self.take(n)?))
    }

    fn check_depth(&self, depth: usize) -> Result<(), ExternalError> {
        if depth > MAX_DEPTH {
            Err(ExternalError::new("Java nesting limit exceeded"))
        } else {
            Ok(())
        }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.pos)
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], ExternalError> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(|| ExternalError::new("truncated Java serialization stream"))?;
        let result = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(result)
    }

    fn u8(&mut self) -> Result<u8, ExternalError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, ExternalError> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().map_err(
            |_| ExternalError::new("truncated Java serialization stream"),
        )?))
    }

    fn u32(&mut self) -> Result<u32, ExternalError> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().map_err(
            |_| ExternalError::new("truncated Java serialization stream"),
        )?))
    }

    fn u64(&mut self) -> Result<u64, ExternalError> {
        Ok(u64::from_be_bytes(self.take(8)?.try_into().map_err(
            |_| ExternalError::new("truncated Java serialization stream"),
        )?))
    }

    fn length_u32(&mut self) -> Result<usize, ExternalError> {
        usize::try_from(self.u32()?)
            .map_err(|_| ExternalError::new("length does not fit this platform"))
    }

    fn length_u64(&mut self) -> Result<usize, ExternalError> {
        usize::try_from(self.u64()?)
            .map_err(|_| ExternalError::new("length does not fit this platform"))
    }

    fn length_i32(&mut self) -> Result<usize, ExternalError> {
        usize::try_from(self.u32()? as i32)
            .map_err(|_| ExternalError::new("negative Java array length"))
    }
}

fn decode_modified_utf8(bytes: &[u8]) -> String {
    let mut units = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b != 0 && b < 0x80 {
            units.push(u16::from(b));
            i += 1;
        } else if b & 0xe0 == 0xc0 && i + 1 < bytes.len() && bytes[i + 1] & 0xc0 == 0x80 {
            let value = (u16::from(b & 0x1f) << 6) | u16::from(bytes[i + 1] & 0x3f);
            if value == 0 || value >= 0x80 {
                units.push(value);
            } else {
                units.push(0xfffd);
            }
            i += 2;
        } else if b & 0xf0 == 0xe0
            && i + 2 < bytes.len()
            && bytes[i + 1] & 0xc0 == 0x80
            && bytes[i + 2] & 0xc0 == 0x80
        {
            let value = (u16::from(b & 0x0f) << 12)
                | (u16::from(bytes[i + 1] & 0x3f) << 6)
                | u16::from(bytes[i + 2] & 0x3f);
            if value >= 0x800 {
                units.push(value);
            } else {
                units.push(0xfffd);
            }
            i += 3;
        } else {
            units.push(0xfffd);
            i += 1;
        }
    }
    std::char::decode_utf16(units)
        .map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream(contents: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0xac, 0xed, 0, 5];
        bytes.extend_from_slice(contents);
        bytes
    }

    #[test]
    fn modified_utf8_decodes_surrogates_malformed_and_nul() {
        assert_eq!(
            decode_modified_utf8(&[0xed, 0xa0, 0xbc, 0xed, 0xbe, 0xa7]),
            "🎧"
        );
        assert_eq!(decode_modified_utf8(&[0xed, 0xa0, 0xbc, b'x']), "�x");
        assert_eq!(decode_modified_utf8(&[0xc0, 0x80]), "\0");
    }

    #[test]
    fn reference_to_earlier_string_resolves_to_same_arena_value() {
        let parsed = read(&stream(&[
            TC_STRING,
            0,
            1,
            b'x',
            TC_REFERENCE,
            0,
            0x7e,
            0,
            0,
        ]))
        .unwrap();
        let refs: Vec<_> = parsed
            .contents
            .iter()
            .filter_map(|c| match c {
                TopContent::Value(Some(r)) => Some(*r),
                _ => None,
            })
            .collect();
        assert_eq!(refs, [0, 0]);
    }

    fn empty_class(name: &str, flags: u8) -> Vec<u8> {
        let mut bytes = vec![TC_CLASSDESC];
        bytes.extend_from_slice(&(name.len() as u16).to_be_bytes());
        bytes.extend_from_slice(name.as_bytes());
        bytes.extend_from_slice(&0_u64.to_be_bytes());
        bytes.extend_from_slice(&[flags, 0, 0, TC_ENDBLOCKDATA, TC_NULL]);
        bytes
    }

    #[test]
    fn reference_to_earlier_object_resolves_to_same_arena_value() {
        let mut body = vec![TC_OBJECT];
        body.extend_from_slice(&empty_class("Thing", SC_SERIALIZABLE));
        body.extend_from_slice(&[TC_REFERENCE, 0, 0x7e, 0, 1]);
        let parsed = read(&stream(&body)).unwrap();
        let refs: Vec<_> = parsed
            .contents
            .iter()
            .filter_map(|c| match c {
                TopContent::Value(Some(r)) => Some(*r),
                _ => None,
            })
            .collect();
        assert_eq!(refs, [1, 1]);
    }

    #[test]
    fn write_method_annotations_are_consumed_before_next_content() {
        let mut body = vec![TC_OBJECT];
        body.extend_from_slice(&empty_class("Annotated", SC_SERIALIZABLE | SC_WRITE_METHOD));
        body.extend_from_slice(&[TC_BLOCKDATA, 1, 42, TC_STRING, 0, 1, b'a', TC_ENDBLOCKDATA]);
        body.extend_from_slice(&[TC_STRING, 0, 1, b'z']);
        let parsed = read(&stream(&body)).unwrap();
        let last = match parsed.contents.last() {
            Some(TopContent::Value(Some(reference))) => parsed.value(*reference).unwrap(),
            _ => panic!("missing top-level string"),
        };
        assert!(matches!(last, Value::String(value) if value == "z"));
    }

    #[test]
    fn reset_clears_handles() {
        let error = read(&stream(&[
            TC_STRING,
            0,
            1,
            b'x',
            TC_RESET,
            TC_REFERENCE,
            0,
            0x7e,
            0,
            0,
        ]))
        .unwrap_err();
        assert!(error.to_string().contains("reference"));
    }

    #[test]
    fn truncated_stream_is_an_error() {
        assert!(read(&stream(&[TC_STRING, 0, 4, b'x'])).is_err());
    }

    #[test]
    fn nesting_beyond_limit_is_an_error() {
        fn nested(depth: usize) -> Vec<u8> {
            let mut bytes = vec![TC_OBJECT, TC_CLASSDESC, 0, 4, b'N', b'o', b'd', b'e'];
            bytes.extend_from_slice(&0_u64.to_be_bytes());
            bytes.extend_from_slice(&[SC_SERIALIZABLE, 0, 1, b'L', 0, 4, b'n', b'e', b'x', b't']);
            bytes.extend_from_slice(&[TC_STRING, 0, 1, b'L', TC_ENDBLOCKDATA, TC_NULL]);
            if depth == 0 {
                bytes.push(TC_NULL);
            } else {
                bytes.extend_from_slice(&nested(depth - 1));
            }
            bytes
        }
        assert!(
            read(&stream(&nested(MAX_DEPTH + 1)))
                .unwrap_err()
                .to_string()
                .contains("nesting")
        );
    }

    #[test]
    fn oversized_array_is_rejected_before_allocation() {
        // [F class descriptor, followed by a length that cannot fit remaining bytes.
        let mut body = vec![TC_ARRAY, TC_CLASSDESC, 0, 2, b'[', b'F'];
        body.extend_from_slice(&0_u64.to_be_bytes());
        body.extend_from_slice(&[SC_SERIALIZABLE, 0, 0, TC_ENDBLOCKDATA, TC_NULL]);
        body.extend_from_slice(&100_i32.to_be_bytes());
        assert!(
            read(&stream(&body))
                .unwrap_err()
                .to_string()
                .contains("remaining bytes")
        );
    }
}
