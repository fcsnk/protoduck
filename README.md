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

### From the DuckDB Community Extensions repository (recommended)

Once ProtoDuck is published to the community extensions registry:

```sql
INSTALL protoduck FROM community;
LOAD protoduck;
```

### Building from source

Prerequisites:

- DuckDB v1.5.2 (the extension is pinned to this release via the unstable C API)
- Rust (stable, 1.80 or later)
- Python 3 (used by the DuckDB extension build tooling and tests)
- `make` and a C toolchain

```bash
# Clone with submodules so the extension-ci-tools build scaffolding is pulled in.
# The Makefile will auto-run `git submodule update --init --recursive` if you
# forget, but `--recursive` on clone is still the cleanest path.
git clone --recursive https://github.com/fcsnk/protoduck
cd protoduck

# 1. Set up the Python venv and detect platform/version metadata.
make configure

# 2. Build the extension (produces build/release/protoduck.duckdb_extension).
make release

# 3. Optional: install into DuckDB's user extension directory so LOAD works
#    with just the extension name (see "Loading the extension" below).
make install
```

Useful development targets:

| Target         | What it does |
| -------------- | ------------ |
| `make debug`   | Debug build, written to `build/debug/protoduck.duckdb_extension` |
| `make test`    | Runs the sqllogictest suite under `test/sql/` against the debug build |
| `make fmt`     | `cargo fmt` |
| `make lint`    | `cargo clippy -- -D warnings` |
| `make check`   | `cargo check` |
| `make clean`   | Remove build artefacts |

A standalone Python smoke test is also included:

```bash
make release
python3 test_extension.py
```

### Loading the extension

Because ProtoDuck is an unsigned extension when built locally, start DuckDB with
the unsigned-extensions flag (CLI) or allow them programmatically:

```bash
duckdb -unsigned
```

```python
con = duckdb.connect(config={"allow_unsigned_extensions": True})
```

After `make install`, the extension lives in DuckDB's per-platform extension
directory (`~/.duckdb/extensions/<platform>/`) and can be loaded by name:

```sql
LOAD protoduck;
```

If you did not run `make install`, load it from its build path:

```sql
LOAD './build/release/protoduck.duckdb_extension';
```

## Usage

### 1. Load your protobuf schema

Register your schema with the extension. You can pass `.proto` source directly:

```sql
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

Or load from a pre-compiled descriptor set produced by
`protoc --descriptor_set_out=schema.desc ...`:

```sql
SELECT proto_schema_add_binary(read_blob('schema.desc'));
```

### 2. Decode protobuf messages

Convert whole protobuf messages to JSON:

```sql
SELECT proto_to_json(protobuf_column, 'myapp.User') AS user_data
FROM my_table;

-- proto_decode is a documented alias of proto_to_json
SELECT proto_decode(protobuf_column, 'myapp.User') AS user_data
FROM my_table;
```

### 3. Extract specific fields

Use dot-notation to extract specific fields:

```sql
-- Simple field
SELECT proto_get(data, 'myapp.User', 'name')           AS user_name FROM my_table;

-- Nested field
SELECT proto_get(data, 'myapp.User', 'address.city')   AS city      FROM my_table;

-- Repeated field by index
SELECT proto_get(data, 'myapp.User', 'tags[0]')        AS first_tag FROM my_table;

-- Map by key
SELECT proto_get(data, 'myapp.Order', 'metadata["key"]') AS meta    FROM my_table;
```

### 4. Inspect a registered schema

```sql
SELECT proto_describe('myapp.User');
```

## Complete example

```sql
LOAD protoduck;

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

CREATE TABLE orders (id INTEGER, data BLOB);

SELECT
    proto_get(data, 'ecommerce.Order', 'order_id')       AS order_id,
    proto_get(data, 'ecommerce.Order', 'customer_name')  AS customer,
    proto_get(data, 'ecommerce.Order', 'status')         AS status,
    proto_get(data, 'ecommerce.Order', 'items[0].name')  AS first_item
FROM orders;

SELECT proto_to_json(data, 'ecommerce.Order') AS order_json
FROM orders;
```

## Functions reference

| Function | Description |
|----------|-------------|
| `proto_schema_add(proto_content VARCHAR)` | Load schema from `.proto` file content |
| `proto_schema_add_binary(descriptor_set BLOB)` | Load schema from a compiled `FileDescriptorSet` |
| `proto_describe(message_type VARCHAR)` | Get a human-readable description of a message type |
| `proto_decode(data BLOB, message_type VARCHAR)` | Decode protobuf to JSON string |
| `proto_to_json(data BLOB, message_type VARCHAR)` | Decode protobuf to JSON string (alias of `proto_decode`) |
| `proto_get(data BLOB, message_type VARCHAR, field_path VARCHAR)` | Extract a specific field value |

## Field path syntax

`proto_get` accepts paths of the form:

- **Simple field**: `field_name`
- **Nested field**: `parent.child.grandchild`
- **Array index**: `items[0]`, `items[42]`
- **Map access**: `metadata["key"]` or `metadata['key']`
- **Combined**: `orders[0].items[1].product.name`

## Type mapping

| Protobuf type           | Output format |
|-------------------------|---------------|
| int32, sint32, sfixed32 | Integer string |
| int64, sint64, sfixed64 | Integer string |
| uint32, fixed32         | Integer string |
| uint64, fixed64         | Integer string |
| float, double           | Decimal string |
| bool                    | `true` or `false` |
| string                  | String value |
| bytes                   | Base64-encoded string |
| enum                    | Enum name (or number if unknown) |
| message                 | JSON object |
| repeated                | JSON array |
| map                     | JSON object |

## Performance considerations

- Schema loading is a one-time operation per session.
- Field extraction with `proto_get` is more efficient than full decode when you only need specific fields.
- For faster schema loading, pre-compile your `.proto` files to a binary `FileDescriptorSet` and use `proto_schema_add_binary`.

## License

MIT

## Contributing

Contributions are welcome — please open issues or pull requests on GitHub.
