# ProtoDuck - DuckDB Protobuf Extension
EXTENSION_NAME=protoduck
TARGET_DUCKDB_VERSION=v1.4.3

# Use unstable C API (required for this DuckDB version)
USE_UNSTABLE_C_API=1

# Include the base and rust makefiles from extension-ci-tools
include extension-ci-tools/makefiles/c_api_extensions/base.Makefile
include extension-ci-tools/makefiles/c_api_extensions/rust.Makefile

.PHONY: all debug release clean configure test

# Default target
all: release

# Configure: set up venv and detect platform
configure: venv platform extension_version
	@echo "Configuration complete!"

# Debug build with metadata
debug: build_extension_with_metadata_debug
	@echo "Debug build complete: build/debug/$(EXTENSION_FILENAME)"

# Release build with metadata
release: build_extension_with_metadata_release
	@echo "Release build complete: build/release/$(EXTENSION_FILENAME)"

# Run Rust unit tests
test_rust:
	cargo test

# Run SQL logic tests (debug)
test_debug: test_extension_debug

# Run SQL logic tests (release)
test_release: test_extension_release

# Alias for test
test: test_rust test_release

# Clean everything
clean: clean_build clean_rust

# Deep clean including configure
clean_all: clean clean_configure

# Format code
fmt:
	cargo fmt

# Lint code
lint:
	cargo clippy -- -D warnings

# Check compilation without building
check:
	cargo check
