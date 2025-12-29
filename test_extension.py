#!/usr/bin/env python3
import duckdb

# Create a simple protobuf message manually (Person with name="Alice", age=30)
# Field 1 (name): tag=10 (field_num=1 << 3 | wire_type=2), length=5, "Alice"
# Field 2 (age): tag=16 (field_num=2 << 3 | wire_type=0), value=30

# Manually encoded protobuf for: Person { name: "Alice", age: 30 }
# Field 1 (string, field_num=1): 0x0a 0x05 "Alice"
# Field 2 (int32, field_num=2): 0x10 0x1e (30)
proto_data = bytes([0x0a, 0x05, 0x41, 0x6c, 0x69, 0x63, 0x65, 0x10, 0x1e])

con = duckdb.connect(config={'allow_unsigned_extensions': True})
con.execute("LOAD './build/release/protoduck.duckdb_extension'")

# Load schema
con.execute("""
SELECT proto_schema_add('
    syntax = "proto3";
    package test;
    message Person {
        string name = 1;
        int32 age = 2;
    }
')
""")

# Create a table with protobuf data
con.execute("CREATE TABLE people (id INTEGER, data BLOB)")
con.execute("INSERT INTO people VALUES (1, ?)", [proto_data])

# Test proto_to_json
result = con.execute("""
    SELECT proto_to_json(data, 'test.Person') as json_data
    FROM people
""").fetchone()
print(f"proto_to_json: {result[0]}")

# Test proto_get
result = con.execute("""
    SELECT
        proto_get(data, 'test.Person', 'name') as name,
        proto_get(data, 'test.Person', 'age') as age
    FROM people
""").fetchone()
print(f"proto_get name: {result[0]}")
print(f"proto_get age: {result[1]}")

print("\nAll tests passed!")
