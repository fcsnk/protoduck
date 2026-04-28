//! Type mapping between Protobuf and DuckDB types
//!
//! This module handles the conversion of protobuf values to their
//! appropriate representations for DuckDB.

use prost_reflect::{DynamicMessage, FieldDescriptor, Kind, MapKey, ReflectMessage, Value};
use serde_json::Value as JsonValue;

use crate::error::{ProtoDuckError, Result};

/// Convert a DynamicMessage to a JSON value
///
/// This handles all protobuf types including nested messages, repeated fields,
/// oneofs, enums, and maps.
pub fn message_to_json(message: &DynamicMessage) -> Result<JsonValue> {
    let mut obj = serde_json::Map::new();
    let descriptor = message.descriptor();

    for field in descriptor.fields() {
        let field_name = field.name().to_string();

        if message.has_field(&field) {
            let value = message.get_field(&field);
            let json_value = value_to_json(&value, &field)?;
            obj.insert(field_name, json_value);
        } else if field.is_list() {
            // Empty list
            obj.insert(field_name, JsonValue::Array(vec![]));
        } else if field.is_map() {
            // Empty map
            obj.insert(field_name, JsonValue::Object(serde_json::Map::new()));
        }
        // For optional fields that aren't set, we skip them (proto3 semantics)
    }

    Ok(JsonValue::Object(obj))
}

/// Convert a protobuf Value to a JSON value
fn value_to_json(value: &Value, field: &prost_reflect::FieldDescriptor) -> Result<JsonValue> {
    match value {
        Value::Bool(b) => Ok(JsonValue::Bool(*b)),
        Value::I32(i) => Ok(JsonValue::Number((*i).into())),
        Value::I64(i) => Ok(JsonValue::Number((*i).into())),
        Value::U32(u) => Ok(JsonValue::Number((*u).into())),
        Value::U64(u) => Ok(JsonValue::Number((*u).into())),
        Value::F32(f) => {
            let n = serde_json::Number::from_f64(*f as f64)
                .unwrap_or_else(|| serde_json::Number::from(0));
            Ok(JsonValue::Number(n))
        }
        Value::F64(f) => {
            let n = serde_json::Number::from_f64(*f).unwrap_or_else(|| serde_json::Number::from(0));
            Ok(JsonValue::Number(n))
        }
        Value::String(s) => Ok(JsonValue::String(s.clone())),
        Value::Bytes(b) => {
            // Encode bytes as base64
            use base64::Engine;
            let encoded = base64::engine::general_purpose::STANDARD.encode(b);
            Ok(JsonValue::String(encoded))
        }
        Value::EnumNumber(n) => {
            // Try to get the enum name, fall back to number
            if let Kind::Enum(enum_desc) = field.kind() {
                if let Some(enum_value) = enum_desc.get_value(*n) {
                    return Ok(JsonValue::String(enum_value.name().to_string()));
                }
            }
            Ok(JsonValue::Number((*n).into()))
        }
        Value::Message(msg) => message_to_json(msg),
        Value::List(list) => {
            let items: Result<Vec<JsonValue>> =
                list.iter().map(|v| value_to_json(v, field)).collect();
            Ok(JsonValue::Array(items?))
        }
        Value::Map(map) => {
            let mut obj = serde_json::Map::new();
            for (key, val) in map.iter() {
                let key_str = map_key_to_string(key);
                let json_val = value_to_json(val, field)?;
                obj.insert(key_str, json_val);
            }
            Ok(JsonValue::Object(obj))
        }
    }
}

/// Convert a map key to a string representation
fn map_key_to_string(key: &MapKey) -> String {
    match key {
        MapKey::Bool(b) => b.to_string(),
        MapKey::I32(i) => i.to_string(),
        MapKey::I64(i) => i.to_string(),
        MapKey::U32(u) => u.to_string(),
        MapKey::U64(u) => u.to_string(),
        MapKey::String(s) => s.clone(),
    }
}

/// Extract a value at a given field path from a message
///
/// Field path supports:
/// - Simple field names: "name"
/// - Nested fields: "user.address.street"
/// - Array indexing: "items[0]"
/// - Map access: "properties['key']" or "properties[\"key\"]"
///
/// Returns the value as a string representation
pub fn extract_field_value(message: &DynamicMessage, path: &str) -> Result<String> {
    let value = navigate_to_value(message, path)?;
    value_to_string(&value)
}

/// Navigate to a value within a message following a field path
fn navigate_to_value(message: &DynamicMessage, path: &str) -> Result<Value> {
    let parts = parse_field_path(path)?;
    let mut current = PathValue::new(Value::Message(message.clone()));

    for part in parts {
        current = apply_path_part(current, &part, path)?;
    }

    Ok(current.value)
}

/// Current value while walking a field path. Map values carry the descriptor for
/// their declared key field so map lookups can parse keys without guessing.
struct PathValue {
    value: Value,
    map_key_field: Option<FieldDescriptor>,
}

impl PathValue {
    fn new(value: Value) -> Self {
        Self {
            value,
            map_key_field: None,
        }
    }

    fn with_map_key(value: Value, map_key_field: Option<FieldDescriptor>) -> Self {
        Self {
            value,
            map_key_field,
        }
    }
}

/// Path part types
#[derive(Debug)]
enum PathPart {
    Field(String),
    Index(usize),
    MapKey(String),
}

/// Parse a field path into parts
fn parse_field_path(path: &str) -> Result<Vec<PathPart>> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut chars = path.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '.' => {
                if !current.is_empty() {
                    parts.push(PathPart::Field(current.clone()));
                    current.clear();
                }
            }
            '[' => {
                if !current.is_empty() {
                    parts.push(PathPart::Field(current.clone()));
                    current.clear();
                }

                // Parse index or map key
                let mut index_str = String::new();

                // Check for quoted key
                if let Some(&next) = chars.peek() {
                    if next == '\'' || next == '"' {
                        let quote_char = chars.next().unwrap();

                        // Read until closing quote
                        for ch in chars.by_ref() {
                            if ch == quote_char {
                                break;
                            }
                            index_str.push(ch);
                        }

                        // Expect closing bracket
                        if chars.next() != Some(']') {
                            return Err(ProtoDuckError::InvalidFieldPath(
                                "Expected ']' after quoted map key".to_string(),
                            ));
                        }

                        parts.push(PathPart::MapKey(index_str));
                        continue;
                    }
                }

                // Read until closing bracket
                for ch in chars.by_ref() {
                    if ch == ']' {
                        break;
                    }
                    index_str.push(ch);
                }

                // Try to parse as integer index
                if let Ok(idx) = index_str.parse::<usize>() {
                    parts.push(PathPart::Index(idx));
                } else {
                    // Treat as map key
                    parts.push(PathPart::MapKey(index_str));
                }
            }
            _ => {
                current.push(c);
            }
        }
    }

    if !current.is_empty() {
        parts.push(PathPart::Field(current));
    }

    if parts.is_empty() {
        return Err(ProtoDuckError::InvalidFieldPath(
            "Empty field path".to_string(),
        ));
    }

    Ok(parts)
}

/// Apply a path part to get the next value
fn apply_path_part(value: PathValue, part: &PathPart, original_path: &str) -> Result<PathValue> {
    let PathValue {
        value,
        map_key_field,
    } = value;

    match (value, part) {
        (Value::Message(msg), PathPart::Field(field_name)) => {
            let descriptor = msg.descriptor();
            let field = descriptor.get_field_by_name(field_name).ok_or_else(|| {
                ProtoDuckError::FieldNotFound(
                    field_name.clone(),
                    descriptor.full_name().to_string(),
                )
            })?;

            // Get the field value (this returns the default if not set)
            let value = msg.get_field(&field).into_owned();
            let map_key_field = map_key_field_descriptor(&field);
            Ok(PathValue::with_map_key(
                resolve_enum_names(value, &field),
                map_key_field,
            ))
        }
        (Value::List(list), PathPart::Index(idx)) => {
            if *idx >= list.len() {
                return Err(ProtoDuckError::IndexOutOfBounds(
                    *idx,
                    original_path.to_string(),
                    list.len(),
                ));
            }
            Ok(PathValue::new(list[*idx].clone()))
        }
        (Value::Map(map), PathPart::MapKey(key)) => {
            let map_key = parse_map_key(key, map_key_field.as_ref(), original_path)?;
            map.get(&map_key)
                .cloned()
                .map(PathValue::new)
                .ok_or_else(|| {
                    ProtoDuckError::MapKeyNotFound(key.clone(), original_path.to_string())
                })
        }
        (Value::Map(map), PathPart::Index(idx)) => {
            let key = idx.to_string();
            let map_key = parse_map_key(&key, map_key_field.as_ref(), original_path)?;
            map.get(&map_key)
                .cloned()
                .map(PathValue::new)
                .ok_or_else(|| ProtoDuckError::MapKeyNotFound(key, original_path.to_string()))
        }
        (Value::List(_), PathPart::Field(f)) => Err(ProtoDuckError::InvalidFieldPath(format!(
            "Cannot access field '{}' on a repeated value - use an index first",
            f
        ))),
        (_, PathPart::Index(_)) => {
            Err(ProtoDuckError::NotARepeatedField(original_path.to_string()))
        }
        (_, PathPart::MapKey(k)) => Err(ProtoDuckError::InvalidFieldPath(format!(
            "Cannot access map key '{}' on non-map value",
            k
        ))),
        (_, PathPart::Field(f)) => Err(ProtoDuckError::InvalidFieldPath(format!(
            "Cannot access field '{}' on non-message value",
            f
        ))),
    }
}

/// Return the descriptor for a protobuf map field's key, if this field is a map.
fn map_key_field_descriptor(field: &FieldDescriptor) -> Option<FieldDescriptor> {
    if let Kind::Message(message) = field.kind() {
        if field.is_map() {
            return Some(message.map_entry_key_field());
        }
    }
    None
}

/// Parse a path key according to the protobuf map's declared key type.
fn parse_map_key(
    key: &str,
    key_field: Option<&FieldDescriptor>,
    original_path: &str,
) -> Result<MapKey> {
    let Some(key_field) = key_field else {
        return Err(ProtoDuckError::InvalidFieldPath(format!(
            "Cannot resolve map key '{}' without map key type metadata in '{}'",
            key, original_path
        )));
    };

    match key_field.kind() {
        Kind::Bool => key
            .parse::<bool>()
            .map(MapKey::Bool)
            .map_err(|_| invalid_map_key(key, key_field, original_path)),
        Kind::Int32 | Kind::Sint32 | Kind::Sfixed32 => key
            .parse::<i32>()
            .map(MapKey::I32)
            .map_err(|_| invalid_map_key(key, key_field, original_path)),
        Kind::Int64 | Kind::Sint64 | Kind::Sfixed64 => key
            .parse::<i64>()
            .map(MapKey::I64)
            .map_err(|_| invalid_map_key(key, key_field, original_path)),
        Kind::Uint32 | Kind::Fixed32 => key
            .parse::<u32>()
            .map(MapKey::U32)
            .map_err(|_| invalid_map_key(key, key_field, original_path)),
        Kind::Uint64 | Kind::Fixed64 => key
            .parse::<u64>()
            .map(MapKey::U64)
            .map_err(|_| invalid_map_key(key, key_field, original_path)),
        Kind::String => Ok(MapKey::String(key.to_string())),
        _ => Err(ProtoDuckError::InvalidFieldPath(format!(
            "Unsupported map key type for '{}' in '{}'",
            key, original_path
        ))),
    }
}

fn invalid_map_key(key: &str, key_field: &FieldDescriptor, original_path: &str) -> ProtoDuckError {
    ProtoDuckError::InvalidFieldPath(format!(
        "Map key '{}' cannot be parsed as {} in '{}'",
        key,
        field_kind_name(key_field),
        original_path
    ))
}

fn field_kind_name(field: &FieldDescriptor) -> &'static str {
    match field.kind() {
        Kind::Double => "double",
        Kind::Float => "float",
        Kind::Int64 => "int64",
        Kind::Uint64 => "uint64",
        Kind::Int32 => "int32",
        Kind::Fixed64 => "fixed64",
        Kind::Fixed32 => "fixed32",
        Kind::Bool => "bool",
        Kind::String => "string",
        Kind::Bytes => "bytes",
        Kind::Uint32 => "uint32",
        Kind::Sfixed32 => "sfixed32",
        Kind::Sfixed64 => "sfixed64",
        Kind::Sint32 => "sint32",
        Kind::Sint64 => "sint64",
        Kind::Enum(_) => "enum",
        Kind::Message(_) => "message",
    }
}

/// Rewrite `EnumNumber` values to their symbolic name (matching `proto_to_json`
/// behavior and the documented type mapping). Applied eagerly while walking a
/// field path, so scalar enum fields, repeated enum fields, and enum-valued
/// map entries all surface the enum's name rather than its wire number.
fn resolve_enum_names(value: Value, field: &FieldDescriptor) -> Value {
    match value {
        Value::EnumNumber(n) => {
            if let Kind::Enum(enum_desc) = field.kind() {
                if let Some(enum_value) = enum_desc.get_value(n) {
                    return Value::String(enum_value.name().to_string());
                }
            }
            Value::EnumNumber(n)
        }
        Value::List(list) => Value::List(
            list.into_iter()
                .map(|v| resolve_enum_names(v, field))
                .collect(),
        ),
        Value::Map(map) => Value::Map(
            map.into_iter()
                .map(|(k, v)| (k, resolve_enum_names(v, field)))
                .collect(),
        ),
        other => other,
    }
}

/// Convert a Value to its string representation
fn value_to_string(value: &Value) -> Result<String> {
    match value {
        Value::Bool(b) => Ok(b.to_string()),
        Value::I32(i) => Ok(i.to_string()),
        Value::I64(i) => Ok(i.to_string()),
        Value::U32(u) => Ok(u.to_string()),
        Value::U64(u) => Ok(u.to_string()),
        Value::F32(f) => Ok(f.to_string()),
        Value::F64(f) => Ok(f.to_string()),
        Value::String(s) => Ok(s.clone()),
        Value::Bytes(b) => {
            use base64::Engine;
            Ok(base64::engine::general_purpose::STANDARD.encode(b))
        }
        Value::EnumNumber(n) => Ok(n.to_string()),
        Value::Message(msg) => {
            let json = message_to_json(msg)?;
            Ok(serde_json::to_string(&json)?)
        }
        Value::List(list) => {
            let items: Result<Vec<String>> = list.iter().map(value_to_string).collect();
            let items = items?;
            Ok(format!("[{}]", items.join(", ")))
        }
        Value::Map(map) => {
            let entries: Result<Vec<String>> = map
                .iter()
                .map(|(k, v)| {
                    let ks = map_key_to_string(k);
                    let vs = value_to_string(v)?;
                    Ok(format!("{}: {}", ks, vs))
                })
                .collect();
            let entries = entries?;
            Ok(format!("{{{}}}", entries.join(", ")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use crate::descriptor_pool::{
        add_schema_from_proto, get_message_descriptor, DescriptorPoolState,
    };

    #[test]
    fn test_parse_simple_path() {
        let parts = parse_field_path("name").unwrap();
        assert_eq!(parts.len(), 1);
        assert!(matches!(&parts[0], PathPart::Field(s) if s == "name"));
    }

    #[test]
    fn test_parse_nested_path() {
        let parts = parse_field_path("user.address.street").unwrap();
        assert_eq!(parts.len(), 3);
    }

    #[test]
    fn test_parse_array_index() {
        let parts = parse_field_path("items[0]").unwrap();
        assert_eq!(parts.len(), 2);
        assert!(matches!(&parts[1], PathPart::Index(0)));
    }

    #[test]
    fn test_parse_map_key() {
        let parts = parse_field_path("properties['key']").unwrap();
        assert_eq!(parts.len(), 2);
        assert!(matches!(&parts[1], PathPart::MapKey(s) if s == "key"));
    }

    #[test]
    fn test_extract_map_key_uses_declared_key_type() {
        let message = map_test_message();

        assert_eq!(
            extract_field_value(&message, "int64_values[1]").unwrap(),
            "signed"
        );
        assert_eq!(
            extract_field_value(&message, "uint64_values[1]").unwrap(),
            "unsigned"
        );
        assert_eq!(
            extract_field_value(&message, "string_values[\"1\"]").unwrap(),
            "string"
        );
    }

    #[test]
    fn test_extract_map_key_rejects_invalid_declared_type() {
        let message = map_test_message();
        let err = extract_field_value(&message, "uint64_values[-1]").unwrap_err();

        assert!(matches!(err, ProtoDuckError::InvalidFieldPath(_)));
    }

    fn map_test_message() -> DynamicMessage {
        let proto = r#"
            syntax = "proto3";
            package maptest;

            message Maps {
                map<int64, string> int64_values = 1;
                map<uint64, string> uint64_values = 2;
                map<string, string> string_values = 3;
            }
        "#;

        let state = DescriptorPoolState::default();
        add_schema_from_proto(&state, proto).unwrap();
        let descriptor = get_message_descriptor(&state, "maptest.Maps").unwrap();
        let mut message = DynamicMessage::new(descriptor);

        message.set_field_by_name(
            "int64_values",
            Value::Map(HashMap::from([(
                MapKey::I64(1),
                Value::String("signed".to_string()),
            )])),
        );
        message.set_field_by_name(
            "uint64_values",
            Value::Map(HashMap::from([(
                MapKey::U64(1),
                Value::String("unsigned".to_string()),
            )])),
        );
        message.set_field_by_name(
            "string_values",
            Value::Map(HashMap::from([(
                MapKey::String("1".to_string()),
                Value::String("string".to_string()),
            )])),
        );

        message
    }
}
