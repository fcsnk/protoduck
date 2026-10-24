//! ProtoDuck - DuckDB Extension for Protobuf Deserialization
//!
//! This extension provides functions to deserialize Protocol Buffer messages
//! stored in database columns, supporting all protobuf data types including
//! oneofs, enums, maps, and nested messages.

mod descriptor_pool;
mod error;
mod type_mapping;

use duckdb_loadable_macros::duckdb_entrypoint_c_api_v2;
use duckdb_neo::{
    connection::Context,
    data_chunk::VectorCollection,
    enums::{FunctionProperty, FunctionPropertyStability},
    extension::Extension,
    scalar::{ScalarCallbacks, ScalarFunctionBuilder},
    signature::{FunctionParameter, SignatureBuilder},
    types::BlobValue,
    vector::{Unknown, Vector},
    DuckDBType,
};

use crate::descriptor_pool::{
    add_schema_from_binary, add_schema_from_proto, decode_message, describe_message_type,
    DescriptorPoolState,
};
use crate::type_mapping::{extract_field_value, message_to_json};

#[derive(Clone, Copy)]
enum Operation {
    SchemaAdd,
    SchemaAddBinary,
    Describe,
    ToJson,
    Get,
}

struct ProtoScalar {
    operation: Operation,
    state: DescriptorPoolState,
}

fn schema_loaded(types: Vec<String>) -> String {
    format!(
        "Loaded {} message type(s): {}",
        types.len(),
        types.join(", ")
    )
}

// Copy each string into DuckDB's output vector before dropping its Rust buffer.
// Invalid schemas, messages, and field paths are query errors, not INTERNAL
// errors (which would invalidate the database under C API v2).
fn write_results(
    output: Vector<'_, Unknown>,
    results: impl ExactSizeIterator<Item = crate::error::Result<Option<String>>>,
) -> duckdb_neo::Result<()> {
    let mut output = output.cast::<String>()?;
    output.set_size(results.len())?;
    let mut writer = output.writer()?;
    for (row, result) in results.enumerate() {
        let value = result.map_err(|err| duckdb_neo::Error::invalid_input(err.to_string()))?;
        writer.set(row, value.as_deref())?;
    }
    Ok(())
}

impl ScalarCallbacks for ProtoScalar {
    type BindData = ();
    type InitData = ();

    fn exec(
        &self,
        _bind_data: Option<&Self::BindData>,
        _init_data: Option<&mut Self::InitData>,
        _context: &Context,
        input: &mut VectorCollection,
        output: Vector<'_, Unknown>,
    ) -> duckdb_neo::Result<()> {
        let state = &self.state;
        match self.operation {
            Operation::SchemaAdd => {
                let content = input.get_vector_at::<String>(0)?.reader()?;
                write_results(
                    output,
                    content.iter().map(|value| {
                        value
                            .map(|value| add_schema_from_proto(state, value).map(schema_loaded))
                            .transpose()
                    }),
                )
            }
            Operation::SchemaAddBinary => {
                let data = input.get_vector_at::<BlobValue>(0)?.reader()?;
                write_results(
                    output,
                    data.iter().map(|value| {
                        value
                            .map(|value| add_schema_from_binary(state, value).map(schema_loaded))
                            .transpose()
                    }),
                )
            }
            Operation::Describe => {
                let message_type = input.get_vector_at::<String>(0)?.reader()?;
                write_results(
                    output,
                    message_type.iter().map(|value| {
                        value
                            .map(|value| describe_message_type(state, value))
                            .transpose()
                    }),
                )
            }
            Operation::ToJson => {
                let data = input.get_vector_at::<BlobValue>(0)?.reader()?;
                let message_type = input.get_vector_at::<String>(1)?.reader()?;
                write_results(
                    output,
                    data.iter()
                        .zip(message_type.iter())
                        .map(|(data, message_type)| match (data, message_type) {
                            (Some(data), Some(message_type)) => {
                                let message = decode_message(state, data, message_type)?;
                                let json = message_to_json(&message)?;
                                Ok(Some(serde_json::to_string(&json)?))
                            }
                            _ => Ok(None),
                        }),
                )
            }
            Operation::Get => {
                let data = input.get_vector_at::<BlobValue>(0)?.reader()?;
                let message_type = input.get_vector_at::<String>(1)?.reader()?;
                let path = input.get_vector_at::<String>(2)?.reader()?;
                write_results(
                    output,
                    data.iter().zip(message_type.iter()).zip(path.iter()).map(
                        |((data, message_type), path)| match (data, message_type, path) {
                            (Some(data), Some(message_type), Some(path)) => {
                                let message = decode_message(state, data, message_type)?;
                                extract_field_value(&message, path).map(Some)
                            }
                            _ => Ok(None),
                        },
                    ),
                )
            }
        }
    }
}

#[duckdb_entrypoint_c_api_v2(extension_name = "protoduck", api_version = "v2.0.0")]
pub fn extension_entrypoint(extension: &Extension) -> duckdb_neo::Result<()> {
    let state = DescriptorPoolState::default();

    for (name, operation) in [
        ("proto_schema_add", Operation::SchemaAdd),
        ("proto_schema_add_binary", Operation::SchemaAddBinary),
        ("proto_describe", Operation::Describe),
        ("proto_to_json", Operation::ToJson),
        ("proto_decode", Operation::ToJson),
        ("proto_get", Operation::Get),
    ] {
        let signature = match operation {
            Operation::SchemaAdd => {
                SignatureBuilder::new().param(FunctionParameter::new::<String>("proto_content"))
            }
            Operation::SchemaAddBinary => {
                SignatureBuilder::new().param(FunctionParameter::new::<BlobValue>("descriptor_set"))
            }
            Operation::Describe => {
                SignatureBuilder::new().param(FunctionParameter::new::<String>("message_type"))
            }
            Operation::ToJson | Operation::Get => {
                let signature = SignatureBuilder::new()
                    .param(FunctionParameter::new::<BlobValue>("data"))
                    .param(FunctionParameter::new::<String>("message_type"));
                if matches!(operation, Operation::Get) {
                    signature.param(FunctionParameter::new::<String>("field_path"))
                } else {
                    signature
                }
            }
        }
        .return_type(String::logical_type(extension)?);

        // Schema registration mutates shared state and must run at execution,
        // including when a prepared statement is executed more than once.
        let stability = match operation {
            Operation::SchemaAdd | Operation::SchemaAddBinary => {
                FunctionPropertyStability::Volatile
            }
            _ => FunctionPropertyStability::ConsistentWithinQuery,
        };
        ScalarFunctionBuilder::new(
            name,
            signature,
            ProtoScalar {
                operation,
                state: state.clone(),
            },
        )
        .set_property(FunctionProperty::Stability(stability))
        .set_property(FunctionProperty::HasSpecialNullHandling(true))
        .register(extension)?;
    }

    Ok(())
}
