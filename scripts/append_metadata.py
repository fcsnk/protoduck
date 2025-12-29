#!/usr/bin/env python3
"""
Append DuckDB extension metadata to a shared library.

This script appends the required metadata footer that DuckDB uses to identify
and validate extensions. Based on the official extension-ci-tools script.
"""

import shutil
import sys
from pathlib import Path


def start_signature():
    """Generate the start signature bytes for DuckDB extension metadata."""
    # This is needed so that Wasm binaries are valid
    encoded_string = b""
    # 0 for custom section
    encoded_string += int(0).to_bytes(1, byteorder="big")
    # 213 in hex = 531 in decimal, total length of what follows (1 + 16 + 2 + 8x32 + 256)
    encoded_string += int(147).to_bytes(1, byteorder="big")
    encoded_string += int(4).to_bytes(1, byteorder="big")
    # 10 in hex = 16 in decimal, length of name, 1 byte
    encoded_string += int(16).to_bytes(1, byteorder="big")
    # the name of the WebAssembly custom section, 16 bytes
    encoded_string += b"duckdb_signature"
    # 1000 in hex, 512 in decimal
    encoded_string += int(128).to_bytes(1, byteorder="big")
    encoded_string += int(4).to_bytes(1, byteorder="big")
    return encoded_string


def padded_byte_string(input_str: str) -> bytes:
    """Pad a string to 32 bytes with null characters."""
    encoded_string = input_str.encode("ascii")
    encoded_string += b"\x00" * (32 - len(encoded_string))
    return encoded_string


def append_metadata(
    input_path: str,
    output_path: str,
    extension_name: str,
    duckdb_version: str,
    platform: str,
    extension_version: str = "dev",
    abi_type: str = "C_STRUCT_UNSTABLE",
):
    """Append DuckDB extension metadata to a library file."""

    input_file = Path(input_path)
    if not input_file.exists():
        print(f"Error: Input file {input_path} not found")
        sys.exit(1)

    output_file = Path(output_path)
    tmp_file = Path(str(output_path) + ".tmp")

    # Copy input to temp file
    shutil.copyfile(input_path, tmp_file)

    print(f"Creating extension: {output_path}")

    # Append metadata
    with open(tmp_file, "ab") as f:
        # Start signature
        f.write(start_signature())

        # FIELD8 (unused)
        f.write(padded_byte_string(""))

        # FIELD7 (unused)
        f.write(padded_byte_string(""))

        # FIELD6 (unused)
        f.write(padded_byte_string(""))

        # FIELD5 (abi_type)
        print(f"  ABI type: {abi_type}")
        f.write(padded_byte_string(abi_type))

        # FIELD4 (extension_version)
        print(f"  Extension version: {extension_version}")
        f.write(padded_byte_string(extension_version))

        # FIELD3 (duckdb_version)
        print(f"  DuckDB version: {duckdb_version}")
        f.write(padded_byte_string(duckdb_version))

        # FIELD2 (platform)
        print(f"  Platform: {platform}")
        f.write(padded_byte_string(platform))

        # FIELD1 (header signature) - the string "4"
        f.write(padded_byte_string("4"))

        # 256 bytes of zeros for the signature
        f.write(b"\x00" * 256)

    # Move temp file to final destination
    shutil.move(tmp_file, output_file)


if __name__ == "__main__":
    if len(sys.argv) < 6:
        print(
            "Usage: append_metadata.py <input> <output> <name> <duckdb_version> <platform> [extension_version]"
        )
        sys.exit(1)

    input_path = sys.argv[1]
    output_path = sys.argv[2]
    extension_name = sys.argv[3]
    duckdb_version = sys.argv[4]
    platform = sys.argv[5]
    extension_version = sys.argv[6] if len(sys.argv) > 6 else "dev"

    append_metadata(
        input_path,
        output_path,
        extension_name,
        duckdb_version,
        platform,
        extension_version,
    )
