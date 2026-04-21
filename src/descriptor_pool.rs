//! Global protobuf descriptor pool management
//!
//! This module provides a thread-safe global descriptor pool that holds
//! all loaded protobuf schemas. Users can add schemas using proto_schema_add()
//! and then decode messages using those schemas.

use once_cell::sync::Lazy;
use parking_lot::RwLock;
use prost::Message;
use prost_reflect::{DescriptorPool, DynamicMessage, MessageDescriptor};
use prost_types::FileDescriptorSet;
use protox::file::{ChainFileResolver, File, FileResolver, GoogleFileResolver};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::error::{ProtoDuckError, Result};

/// Global descriptor pool that holds all loaded protobuf schemas
static DESCRIPTOR_POOL: Lazy<RwLock<DescriptorPool>> = Lazy::new(|| {
    // Start with an empty pool, but include well-known types
    let pool = DescriptorPool::global();
    RwLock::new(pool.clone())
});

/// Counter for generating unique file names
static FILE_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Custom file resolver that serves inline proto content
struct InlineFileResolver {
    filename: String,
    content: String,
}

impl FileResolver for InlineFileResolver {
    fn open_file(&self, name: &str) -> std::result::Result<File, protox::Error> {
        if name == self.filename {
            File::from_source(&self.filename, &self.content)
        } else {
            Err(protox::Error::file_not_found(name))
        }
    }
}

/// Add a proto schema from its text representation (.proto file content)
///
/// This parses the proto file content and adds all message types to the global pool.
pub fn add_schema_from_proto(proto_content: &str) -> Result<Vec<String>> {
    // Generate a unique filename for this inline content
    let file_num = FILE_COUNTER.fetch_add(1, Ordering::SeqCst);
    let filename = format!("inline_{}.proto", file_num);

    // Create a resolver chain: inline content first, then Google well-known types
    let inline_resolver = InlineFileResolver {
        filename: filename.clone(),
        content: proto_content.to_string(),
    };
    let mut resolver = ChainFileResolver::new();
    resolver.add(inline_resolver);
    resolver.add(GoogleFileResolver::new());

    // Compile using the custom resolver
    let mut compiler = protox::Compiler::with_file_resolver(resolver);
    compiler.include_imports(true);
    compiler
        .open_file(&filename)
        .map_err(|e| ProtoDuckError::SchemaParseError(e.to_string()))?;

    let file_descriptor_set = compiler.file_descriptor_set();

    add_file_descriptor_set(file_descriptor_set)
}

/// Add a proto schema from a compiled FileDescriptorSet (binary format)
///
/// This is useful when you have pre-compiled .desc files from protoc.
pub fn add_schema_from_binary(data: &[u8]) -> Result<Vec<String>> {
    let file_descriptor_set = FileDescriptorSet::decode(data)?;
    add_file_descriptor_set(file_descriptor_set)
}

/// Add a FileDescriptorSet to the global pool
fn add_file_descriptor_set(fds: FileDescriptorSet) -> Result<Vec<String>> {
    let mut pool = DESCRIPTOR_POOL.write();

    // Create a new pool that includes the existing pool plus new descriptors
    let mut new_pool = pool.clone();
    new_pool.add_file_descriptor_set(fds)?;

    // Collect all message type names that were added
    let message_names: Vec<String> = new_pool
        .all_messages()
        .map(|m| m.full_name().to_string())
        .collect();

    *pool = new_pool;

    Ok(message_names)
}

/// Get a message descriptor by its fully qualified name
///
/// The name can be with or without a leading dot.
pub fn get_message_descriptor(message_type: &str) -> Result<MessageDescriptor> {
    let pool = DESCRIPTOR_POOL.read();

    // Try with and without leading dot
    let type_name = message_type.trim_start_matches('.');

    pool.get_message_by_name(type_name)
        .ok_or_else(|| ProtoDuckError::MessageTypeNotFound(type_name.to_string()))
}

/// Decode a protobuf message from binary data
pub fn decode_message(data: &[u8], message_type: &str) -> Result<DynamicMessage> {
    let descriptor = get_message_descriptor(message_type)?;
    let message = DynamicMessage::decode(descriptor, data)?;
    Ok(message)
}

/// Get a formatted description of a message type
pub fn describe_message_type(message_type: &str) -> Result<String> {
    let descriptor = get_message_descriptor(message_type)?;
    let mut description = String::new();

    description.push_str(&format!("message {} {{\n", descriptor.name()));

    // Describe regular fields
    for field in descriptor.fields() {
        let type_name = format_field_type(&field);
        let label = if field.is_list() {
            "repeated "
        } else if field.cardinality() == prost_reflect::Cardinality::Optional {
            "optional "
        } else {
            ""
        };

        description.push_str(&format!(
            "  {}{} {} = {};\n",
            label,
            type_name,
            field.name(),
            field.number()
        ));
    }

    // Describe oneofs (only real oneofs, not synthetic ones from optional fields)
    for oneof in descriptor.oneofs() {
        // Skip oneofs with only one field (these are synthetic from proto3 optional)
        let fields: Vec<_> = oneof.fields().collect();
        if fields.len() > 1 {
            description.push_str(&format!("  oneof {} {{\n", oneof.name()));
            for field in fields {
                let type_name = format_field_type(&field);
                description.push_str(&format!(
                    "    {} {} = {};\n",
                    type_name,
                    field.name(),
                    field.number()
                ));
            }
            description.push_str("  }\n");
        }
    }

    description.push('}');

    Ok(description)
}

/// Format a field type for display
fn format_field_type(field: &prost_reflect::FieldDescriptor) -> String {
    use prost_reflect::Kind;

    match field.kind() {
        Kind::Double => "double".to_string(),
        Kind::Float => "float".to_string(),
        Kind::Int64 => "int64".to_string(),
        Kind::Uint64 => "uint64".to_string(),
        Kind::Int32 => "int32".to_string(),
        Kind::Fixed64 => "fixed64".to_string(),
        Kind::Fixed32 => "fixed32".to_string(),
        Kind::Bool => "bool".to_string(),
        Kind::String => "string".to_string(),
        Kind::Bytes => "bytes".to_string(),
        Kind::Uint32 => "uint32".to_string(),
        Kind::Sfixed32 => "sfixed32".to_string(),
        Kind::Sfixed64 => "sfixed64".to_string(),
        Kind::Sint32 => "sint32".to_string(),
        Kind::Sint64 => "sint64".to_string(),
        Kind::Enum(e) => format!("enum {}", e.name()),
        Kind::Message(m) => {
            if field.is_map() {
                let key_field = m.map_entry_key_field();
                let value_field = m.map_entry_value_field();
                format!(
                    "map<{}, {}>",
                    format_field_type(&key_field),
                    format_field_type(&value_field)
                )
            } else {
                m.name().to_string()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_get_schema() {
        let proto = r#"
            syntax = "proto3";
            package test;

            message Person {
                string name = 1;
                int32 age = 2;
            }
        "#;

        let types = add_schema_from_proto(proto).unwrap();
        assert!(types.iter().any(|t| t.contains("Person")));

        let descriptor = get_message_descriptor("test.Person").unwrap();
        assert_eq!(descriptor.name(), "Person");
    }
}
