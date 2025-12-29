# ProtoDuck - DuckDB Protobuf Extension
# Standalone Makefile (no submodules required)

EXTENSION_NAME=protoduck
TARGET_DUCKDB_VERSION=v1.4.3

# Platform detection
UNAME_S := $(shell uname -s)
ifeq ($(UNAME_S),Linux)
    EXTENSION_LIB=lib$(EXTENSION_NAME).so
    PLATFORM=linux_amd64
endif
ifeq ($(UNAME_S),Darwin)
    EXTENSION_LIB=lib$(EXTENSION_NAME).dylib
    UNAME_M := $(shell uname -m)
    ifeq ($(UNAME_M),arm64)
        PLATFORM=osx_arm64
    else
        PLATFORM=osx_amd64
    endif
endif

EXTENSION_FILE=$(EXTENSION_NAME).duckdb_extension
PYTHON_BIN?=python3

.PHONY: all debug release clean configure test install

# Default target
all: release

# Configure: set up venv
configure:
	@echo "Setting up Python virtual environment..."
	@mkdir -p configure
	$(PYTHON_BIN) -m venv configure/venv
	./configure/venv/bin/pip install --quiet --upgrade pip
	./configure/venv/bin/pip install --quiet duckdb packaging
	@echo "$(PLATFORM)" > configure/platform.txt
	@git describe --tags --always 2>/dev/null > configure/extension_version.txt || echo "dev" > configure/extension_version.txt
	@echo "Configuration complete!"

# Check if configured
check_configure:
	@test -d configure/venv || (echo "Please run 'make configure' first" && exit 1)

# Debug build
debug: check_configure
	@echo "Building debug..."
	cargo build
	@mkdir -p build/debug/extension/$(EXTENSION_NAME)
	@cp target/debug/$(EXTENSION_LIB) build/debug/$(EXTENSION_LIB)
	@./configure/venv/bin/python3 scripts/append_metadata.py \
		build/debug/$(EXTENSION_LIB) \
		build/debug/$(EXTENSION_FILE) \
		$(EXTENSION_NAME) \
		$(TARGET_DUCKDB_VERSION) \
		$(PLATFORM)
	@cp build/debug/$(EXTENSION_FILE) build/debug/extension/$(EXTENSION_NAME)/
	@echo "Debug build complete: build/debug/$(EXTENSION_FILE)"

# Release build
release: check_configure
	@echo "Building release..."
	cargo build --release
	@mkdir -p build/release/extension/$(EXTENSION_NAME)
	@cp target/release/$(EXTENSION_LIB) build/release/$(EXTENSION_LIB)
	@./configure/venv/bin/python3 scripts/append_metadata.py \
		build/release/$(EXTENSION_LIB) \
		build/release/$(EXTENSION_FILE) \
		$(EXTENSION_NAME) \
		$(TARGET_DUCKDB_VERSION) \
		$(PLATFORM)
	@cp build/release/$(EXTENSION_FILE) build/release/extension/$(EXTENSION_NAME)/
	@echo "Release build complete: build/release/$(EXTENSION_FILE)"

# Run Rust unit tests
test:
	cargo test

# Clean build artifacts
clean:
	cargo clean
	rm -rf build/

# Deep clean including configure
clean_all: clean
	rm -rf configure/

# Format code
fmt:
	cargo fmt

# Lint code
lint:
	cargo clippy -- -D warnings

# Check compilation without building
check:
	cargo check

# Install to user's DuckDB extensions directory
install: release
	@mkdir -p ~/.duckdb/extensions/$(PLATFORM)
	@cp build/release/$(EXTENSION_FILE) ~/.duckdb/extensions/$(PLATFORM)/
	@echo "Installed to ~/.duckdb/extensions/$(PLATFORM)/$(EXTENSION_FILE)"
