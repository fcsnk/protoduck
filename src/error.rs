//! Error types for the ProtoDuck extension

use thiserror::Error;

/// Errors that can occur in the ProtoDuck extension
#[derive(Error, Debug)]
pub enum ProtoDuckError {
    /// Failed to decode protobuf message
    #[error("Failed to decode protobuf message: {0}")]
    DecodeError(String),

    /// Message type not found in descriptor pool
    #[error("Message type '{0}' not found. Did you load the schema with proto_schema_add()?")]
    MessageTypeNotFound(String),

    /// Field not found in message
    #[error("Field '{0}' not found in message type '{1}'")]
    FieldNotFound(String, String),

    /// Invalid field path syntax
    #[error("Invalid field path syntax: {0}")]
    InvalidFieldPath(String),

    /// Failed to parse proto schema
    #[error("Failed to parse proto schema: {0}")]
    SchemaParseError(String),

    /// Failed to serialize to JSON
    #[error("Failed to serialize to JSON: {0}")]
    JsonSerializeError(String),

    /// Index out of bounds for repeated field
    #[error("Index {0} out of bounds for repeated field '{1}' (length: {2})")]
    IndexOutOfBounds(usize, String, usize),

    /// Attempted to index a non-repeated field
    #[error("Cannot use array index on non-repeated field '{0}'")]
    NotARepeatedField(String),

    /// Map key not found
    #[error("Map key '{0}' not found in field '{1}'")]
    MapKeyNotFound(String, String),
}

impl From<prost::DecodeError> for ProtoDuckError {
    fn from(err: prost::DecodeError) -> Self {
        ProtoDuckError::DecodeError(err.to_string())
    }
}

impl From<prost_reflect::DescriptorError> for ProtoDuckError {
    fn from(err: prost_reflect::DescriptorError) -> Self {
        ProtoDuckError::SchemaParseError(err.to_string())
    }
}

impl From<serde_json::Error> for ProtoDuckError {
    fn from(err: serde_json::Error) -> Self {
        ProtoDuckError::JsonSerializeError(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, ProtoDuckError>;
