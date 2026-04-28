//! ProtoDuck - DuckDB Extension for Protobuf Deserialization
//!
//! This extension provides functions to deserialize Protocol Buffer messages
//! stored in database columns, supporting all protobuf data types including
//! oneofs, enums, maps, and nested messages.

mod descriptor_pool;
mod error;
mod type_mapping;

use std::sync::Arc;

use duckdb::arrow::array::{Array, BinaryArray, RecordBatch, StringArray};
use duckdb::arrow::datatypes::DataType;
use duckdb::vscalar::arrow::{ArrowFunctionSignature, VArrowScalar};
use duckdb::{duckdb_entrypoint_c_api, Connection};

use crate::descriptor_pool::{
    add_schema_from_binary, add_schema_from_proto, decode_message, describe_message_type,
    DescriptorPoolState,
};
use crate::type_mapping::{extract_field_value, message_to_json};

// ============================================================================
// Proto Schema Add Function
// ============================================================================

struct ProtoSchemaAdd;

impl VArrowScalar for ProtoSchemaAdd {
    type State = DescriptorPoolState;

    fn invoke(
        state: &Self::State,
        input: RecordBatch,
    ) -> Result<Arc<dyn Array>, Box<dyn std::error::Error>> {
        let content_col = input
            .column(0)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or("Expected string array for proto content")?;

        let results: Vec<String> = content_col
            .iter()
            .map(|content| match content {
                Some(proto_content) => match add_schema_from_proto(state, proto_content) {
                    Ok(types) => format!(
                        "Loaded {} message type(s): {}",
                        types.len(),
                        types.join(", ")
                    ),
                    Err(e) => format!("Error: {}", e),
                },
                None => "Error: NULL input".to_string(),
            })
            .collect();

        Ok(Arc::new(StringArray::from(results)))
    }

    fn signatures() -> Vec<ArrowFunctionSignature> {
        vec![ArrowFunctionSignature::exact(
            vec![DataType::Utf8],
            DataType::Utf8,
        )]
    }
}

// ============================================================================
// Proto Schema Add Binary Function
// ============================================================================

struct ProtoSchemaAddBinary;

impl VArrowScalar for ProtoSchemaAddBinary {
    type State = DescriptorPoolState;

    fn invoke(
        state: &Self::State,
        input: RecordBatch,
    ) -> Result<Arc<dyn Array>, Box<dyn std::error::Error>> {
        let blob_col = input
            .column(0)
            .as_any()
            .downcast_ref::<BinaryArray>()
            .ok_or("Expected binary array for descriptor set")?;

        let results: Vec<String> = blob_col
            .iter()
            .map(|blob| match blob {
                Some(data) => match add_schema_from_binary(state, data) {
                    Ok(types) => format!(
                        "Loaded {} message type(s): {}",
                        types.len(),
                        types.join(", ")
                    ),
                    Err(e) => format!("Error: {}", e),
                },
                None => "Error: NULL input".to_string(),
            })
            .collect();

        Ok(Arc::new(StringArray::from(results)))
    }

    fn signatures() -> Vec<ArrowFunctionSignature> {
        vec![ArrowFunctionSignature::exact(
            vec![DataType::Binary],
            DataType::Utf8,
        )]
    }
}

// ============================================================================
// Proto Describe Function
// ============================================================================

struct ProtoDescribe;

impl VArrowScalar for ProtoDescribe {
    type State = DescriptorPoolState;

    fn invoke(
        state: &Self::State,
        input: RecordBatch,
    ) -> Result<Arc<dyn Array>, Box<dyn std::error::Error>> {
        let type_col = input
            .column(0)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or("Expected string array for message type")?;

        let results: Vec<String> = type_col
            .iter()
            .map(|message_type| match message_type {
                Some(mt) => match describe_message_type(state, mt) {
                    Ok(desc) => desc,
                    Err(e) => format!("Error: {}", e),
                },
                None => "Error: NULL input".to_string(),
            })
            .collect();

        Ok(Arc::new(StringArray::from(results)))
    }

    fn signatures() -> Vec<ArrowFunctionSignature> {
        vec![ArrowFunctionSignature::exact(
            vec![DataType::Utf8],
            DataType::Utf8,
        )]
    }
}

// ============================================================================
// Proto To JSON Function
// ============================================================================

struct ProtoToJson;

impl VArrowScalar for ProtoToJson {
    type State = DescriptorPoolState;

    fn invoke(
        state: &Self::State,
        input: RecordBatch,
    ) -> Result<Arc<dyn Array>, Box<dyn std::error::Error>> {
        let blob_col = input
            .column(0)
            .as_any()
            .downcast_ref::<BinaryArray>()
            .ok_or("Expected binary array for protobuf data")?;

        let type_col = input
            .column(1)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or("Expected string array for message type")?;

        let results: Vec<Option<String>> = blob_col
            .iter()
            .zip(type_col.iter())
            .map(
                |(blob, message_type)| -> Result<Option<String>, crate::error::ProtoDuckError> {
                    match (blob, message_type) {
                        (Some(data), Some(mt)) => decode_message(state, data, mt)
                            .and_then(|msg| message_to_json(&msg))
                            .and_then(|json| {
                                serde_json::to_string(&json)
                                    .map_err(crate::error::ProtoDuckError::from)
                            })
                            .map(Some),
                        _ => Ok(None),
                    }
                },
            )
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Arc::new(StringArray::from(results)))
    }

    fn signatures() -> Vec<ArrowFunctionSignature> {
        vec![ArrowFunctionSignature::exact(
            vec![DataType::Binary, DataType::Utf8],
            DataType::Utf8,
        )]
    }
}

// ============================================================================
// Proto Get Function
// ============================================================================

struct ProtoGet;

impl VArrowScalar for ProtoGet {
    type State = DescriptorPoolState;

    fn invoke(
        state: &Self::State,
        input: RecordBatch,
    ) -> Result<Arc<dyn Array>, Box<dyn std::error::Error>> {
        let blob_col = input
            .column(0)
            .as_any()
            .downcast_ref::<BinaryArray>()
            .ok_or("Expected binary array for protobuf data")?;

        let type_col = input
            .column(1)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or("Expected string array for message type")?;

        let path_col = input
            .column(2)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or("Expected string array for field path")?;

        let results: Vec<Option<String>> = blob_col
            .iter()
            .zip(type_col.iter())
            .zip(path_col.iter())
            .map(
                |((blob, message_type), field_path)| -> Result<
                    Option<String>,
                    crate::error::ProtoDuckError,
                > {
                    match (blob, message_type, field_path) {
                        (Some(data), Some(mt), Some(path)) => decode_message(state, data, mt)
                            .and_then(|msg| extract_field_value(&msg, path))
                            .map(Some),
                        _ => Ok(None),
                    }
                },
            )
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Arc::new(StringArray::from(results)))
    }

    fn signatures() -> Vec<ArrowFunctionSignature> {
        vec![ArrowFunctionSignature::exact(
            vec![DataType::Binary, DataType::Utf8, DataType::Utf8],
            DataType::Utf8,
        )]
    }
}

// ============================================================================
// Extension Entry Point
// ============================================================================

#[duckdb_entrypoint_c_api(ext_name = "protoduck", min_duckdb_version = "v1.5.2")]
pub unsafe fn extension_entrypoint(con: Connection) -> Result<(), Box<dyn std::error::Error>> {
    let descriptor_state = DescriptorPoolState::default();

    // Register all scalar functions
    con.register_scalar_function_with_state::<ProtoSchemaAdd>(
        "proto_schema_add",
        &descriptor_state,
    )?;
    con.register_scalar_function_with_state::<ProtoSchemaAddBinary>(
        "proto_schema_add_binary",
        &descriptor_state,
    )?;
    con.register_scalar_function_with_state::<ProtoDescribe>("proto_describe", &descriptor_state)?;
    con.register_scalar_function_with_state::<ProtoToJson>("proto_to_json", &descriptor_state)?;
    con.register_scalar_function_with_state::<ProtoToJson>("proto_decode", &descriptor_state)?; // alias
    con.register_scalar_function_with_state::<ProtoGet>("proto_get", &descriptor_state)?;

    Ok(())
}
