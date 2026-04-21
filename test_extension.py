#!/usr/bin/env python3
"""Smoke tests for the ProtoDuck extension.

Run `make release` first so that the built extension exists at
./build/release/protoduck.duckdb_extension. The script asserts expected values
and exits non-zero on failure so it can be wired into CI.
"""
import sys

import duckdb


def encode_varint(value: int) -> bytes:
    out = bytearray()
    while value > 0x7F:
        out.append((value & 0x7F) | 0x80)
        value >>= 7
    out.append(value & 0x7F)
    return bytes(out)


def tag(field_num: int, wire_type: int) -> bytes:
    return encode_varint((field_num << 3) | wire_type)


def length_delimited(field_num: int, payload: bytes) -> bytes:
    return tag(field_num, 2) + encode_varint(len(payload)) + payload


def varint_field(field_num: int, value: int) -> bytes:
    return tag(field_num, 0) + encode_varint(value)


def string_field(field_num: int, value: str) -> bytes:
    return length_delimited(field_num, value.encode("utf-8"))


def assert_eq(label: str, actual, expected) -> None:
    if actual != expected:
        print(f"FAIL {label}: expected {expected!r}, got {actual!r}")
        sys.exit(1)
    print(f"ok   {label}: {actual!r}")


def assert_contains(label: str, haystack: str, needle: str) -> None:
    if needle not in haystack:
        print(f"FAIL {label}: {needle!r} not in {haystack!r}")
        sys.exit(1)
    print(f"ok   {label}: contains {needle!r}")


con = duckdb.connect(config={"allow_unsigned_extensions": True})
con.execute("LOAD './build/release/protoduck.duckdb_extension'")

con.execute(
    """
    SELECT proto_schema_add('
        syntax = "proto3";
        package test;
        enum Status { UNKNOWN = 0; ACTIVE = 1; INACTIVE = 2; }
        message Address { string city = 1; string country = 2; }
        message Person {
            string name = 1;
            int32  age  = 2;
            Status status = 3;
            Address address = 4;
            repeated string tags = 5;
            map<string, string> attributes = 6;
        }
    ')
    """
)

# --- scalar + nested + repeated + enum + map payload -----------------------
address = string_field(1, "Portland") + string_field(2, "US")
attr_entry = string_field(1, "team") + string_field(2, "duckdb")

payload = (
    string_field(1, "Alice")
    + varint_field(2, 30)
    + varint_field(3, 1)  # Status.ACTIVE
    + length_delimited(4, address)
    + string_field(5, "engineer")
    + string_field(5, "rustacean")
    + length_delimited(6, attr_entry)
)

con.execute("CREATE TABLE people (id INTEGER, data BLOB)")
con.execute("INSERT INTO people VALUES (1, ?)", [payload])

# proto_to_json round-trip
json_blob = con.execute(
    "SELECT proto_to_json(data, 'test.Person') FROM people"
).fetchone()[0]
assert_contains("proto_to_json name", json_blob, '"Alice"')
assert_contains("proto_to_json enum", json_blob, "ACTIVE")
assert_contains("proto_to_json nested", json_blob, "Portland")

# Scalar field
name, age = con.execute(
    "SELECT proto_get(data, 'test.Person', 'name'), proto_get(data, 'test.Person', 'age') FROM people"
).fetchone()
assert_eq("proto_get name", name, "Alice")
assert_eq("proto_get age", age, "30")

# Enum renders as its name
status = con.execute(
    "SELECT proto_get(data, 'test.Person', 'status') FROM people"
).fetchone()[0]
assert_eq("proto_get enum", status, "ACTIVE")

# Nested message field
city = con.execute(
    "SELECT proto_get(data, 'test.Person', 'address.city') FROM people"
).fetchone()[0]
assert_eq("proto_get nested", city, "Portland")

# Repeated field indexing
first_tag = con.execute(
    "SELECT proto_get(data, 'test.Person', 'tags[0]') FROM people"
).fetchone()[0]
second_tag = con.execute(
    "SELECT proto_get(data, 'test.Person', 'tags[1]') FROM people"
).fetchone()[0]
assert_eq("proto_get tags[0]", first_tag, "engineer")
assert_eq("proto_get tags[1]", second_tag, "rustacean")

# Map access
team = con.execute(
    "SELECT proto_get(data, 'test.Person', 'attributes[\"team\"]') FROM people"
).fetchone()[0]
assert_eq("proto_get map", team, "duckdb")

# proto_describe returns something non-empty for a registered message
description = con.execute("SELECT proto_describe('test.Person')").fetchone()[0]
if not description:
    print("FAIL proto_describe returned empty result")
    sys.exit(1)
print(f"ok   proto_describe: {len(description)} chars")

print("\nAll tests passed!")
