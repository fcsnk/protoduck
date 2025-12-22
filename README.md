# ProtoDuck

A DuckDB extension for deserializing Protocol Buffer messages stored in database columns.

Unlike file-based protobuf extensions, ProtoDuck operates on serialized protobuf data stored directly in table columns, making it ideal for working with protobuf-encoded data in data lakes and analytics pipelines.

## Features

- **Dynamic Schema Loading**: Load `.proto` schemas at runtime without recompilation
- **Full Type Support**: All protobuf types including:
  - Scalar types (int32, int64, string, bytes, bool, float, double, etc.)
  - Nested messages
  - Repeated fields (arrays)
  - Maps
  - Oneofs
  - Enums
- **Field Extraction**: Extract specific fields using dot-notation paths
- **JSON Output**: Convert protobuf messages to JSON for easy analysis

## Installation

### Building from Source

Prerequisites:
- Rust 1.74 or later
- Python 3 (for testing)

```bash
# Clone the repository
git clone https://github.com/fcsnk/protoduck
cd protoduck

# Build release version
make release

# Optional: Run tests
make configure  # Set up Python venv
make test       # Run Rust unit tests
```

### Loading the Extension

```sql
-- Load the extension (use -unsigned flag when starting DuckDB)
LOAD 'path/to/protoduck.duckdb_extension';
```

## Usage

### 1. Load Your Protobuf Schema

First, register your protobuf schema with the extension:

```sql
-- Load schema from .proto content
SELECT proto_schema_add('
    syntax = "proto3";
    package myapp;

    message User {
        int32 id = 1;
        string name = 2;
        string email = 3;
        repeated string tags = 4;
        Address address = 5;
    }

    message Address {
        string street = 1;
        string city = 2;
        string country = 3;
    }
');
```

Alternatively, load from a pre-compiled descriptor set:

```sql
-- Load from binary FileDescriptorSet (created by protoc --descriptor_set_out)
SELECT proto_schema_add_binary(read_blob('schema.desc'));
```

### 2. Decode Protobuf Messages

Convert entire protobuf messages to JSON:

```sql
-- Decode protobuf blob to JSON
SELECT proto_to_json(protobuf_column, 'myapp.User') as user_data
FROM my_table;

-- Or use proto_decode (alias)
SELECT proto_decode(protobuf_column, 'myapp.User') as user_data
FROM my_table;
```

### 3. Extract Specific Fields

Use dot-notation to extract specific fields:

```sql
-- Extract simple fields
SELECT proto_get(data, 'myapp.User', 'name') as user_name
FROM my_table;

-- Extract nested fields
SELECT proto_get(data, 'myapp.User', 'address.city') as city
FROM my_table;

-- Extract from repeated fields (array indexing)
SELECT proto_get(data, 'myapp.User', 'tags[0]') as first_tag
FROM my_table;

-- Extract from maps
SELECT proto_get(data, 'myapp.Order', 'metadata["key"]') as meta_value
FROM my_table;
```

### 4. Inspect Schema

View the structure of a registered message type:

```sql
SELECT proto_describe('myapp.User');
```

## Complete Example

```sql
-- Load the extension
LOAD 'protoduck.duckdb_extension';

-- Define schema
SELECT proto_schema_add('
    syntax = "proto3";
    package ecommerce;

    enum OrderStatus {
        PENDING = 0;
        SHIPPED = 1;
        DELIVERED = 2;
    }

    message Order {
        int64 order_id = 1;
        string customer_name = 2;
        repeated Item items = 3;
        OrderStatus status = 4;
        map<string, string> metadata = 5;
    }

    message Item {
        string product_id = 1;
        string name = 2;
        int32 quantity = 3;
        double price = 4;
    }
');

-- Create a table with protobuf data
CREATE TABLE orders (
    id INTEGER,
    data BLOB
);

-- Query protobuf data
SELECT
    proto_get(data, 'ecommerce.Order', 'order_id') as order_id,
    proto_get(data, 'ecommerce.Order', 'customer_name') as customer,
    proto_get(data, 'ecommerce.Order', 'status') as status,
    proto_get(data, 'ecommerce.Order', 'items[0].name') as first_item
FROM orders;

-- Get full JSON representation
SELECT proto_to_json(data, 'ecommerce.Order') as order_json
FROM orders;
```

## Functions Reference

| Function | Description |
|----------|-------------|
| `proto_schema_add(proto_content VARCHAR)` | Load schema from .proto file content |
| `proto_schema_add_binary(descriptor_set BLOB)` | Load schema from compiled FileDescriptorSet |
| `proto_describe(message_type VARCHAR)` | Get human-readable description of a message type |
| `proto_decode(data BLOB, message_type VARCHAR)` | Decode protobuf to JSON string |
| `proto_to_json(data BLOB, message_type VARCHAR)` | Decode protobuf to JSON string (alias) |
| `proto_get(data BLOB, message_type VARCHAR, field_path VARCHAR)` | Extract a specific field value |

## Field Path Syntax

The `proto_get` function supports a flexible path syntax:

- **Simple field**: `field_name`
- **Nested field**: `parent.child.grandchild`
- **Array index**: `items[0]`, `items[42]`
- **Map access**: `metadata["key"]` or `metadata['key']`
- **Combined**: `orders[0].items[1].product.name`

## Type Mapping

| Protobuf Type | Output Format |
|---------------|---------------|
| int32, sint32, sfixed32 | Integer string |
| int64, sint64, sfixed64 | Integer string |
| uint32, fixed32 | Integer string |
| uint64, fixed64 | Integer string |
| float, double | Decimal string |
| bool | "true" or "false" |
| string | String value |
| bytes | Base64-encoded string |
| enum | Enum name (or number if unknown) |
| message | JSON object |
| repeated | JSON array |
| map | JSON object |

## Performance Considerations

- Schema loading is a one-time operation per session
- Field extraction with `proto_get` is more efficient than full decode when you only need specific fields
- Consider pre-compiling your `.proto` files to binary descriptor sets for faster loading

## License

MIT

## Contributing

Contributions are welcome! Please feel free to submit issues and pull requests.
