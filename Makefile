.PHONY: all debug release clean configure test test_debug test_release install

# Default target
all: release

# Configuration - set up Python venv and DuckDB
configure:
	@echo "Setting up Python virtual environment..."
	python3 -m venv .venv
	.venv/bin/pip install --upgrade pip
	.venv/bin/pip install duckdb
	@echo "Configuration complete!"

# Debug build
debug:
	@echo "Building debug..."
	cargo build
	@mkdir -p build/debug
	@cp target/debug/libprotoduck.so build/debug/protoduck.duckdb_extension 2>/dev/null || \
		cp target/debug/libprotoduck.dylib build/debug/protoduck.duckdb_extension 2>/dev/null || \
		cp target/debug/protoduck.dll build/debug/protoduck.duckdb_extension 2>/dev/null || \
		echo "Built library, copy manually if needed"
	@echo "Debug build complete: build/debug/protoduck.duckdb_extension"

# Release build
release:
	@echo "Building release..."
	cargo build --release
	@mkdir -p build/release
	@cp target/release/libprotoduck.so build/release/protoduck.duckdb_extension 2>/dev/null || \
		cp target/release/libprotoduck.dylib build/release/protoduck.duckdb_extension 2>/dev/null || \
		cp target/release/protoduck.dll build/release/protoduck.duckdb_extension 2>/dev/null || \
		echo "Built library, copy manually if needed"
	@echo "Release build complete: build/release/protoduck.duckdb_extension"

# Run Rust tests
test:
	cargo test

# Run SQL tests with debug build
test_debug: debug
	@echo "Running SQL tests with debug build..."
	@for f in test/sql/*.test; do \
		if [ -f "$$f" ]; then \
			echo "Running $$f..."; \
			.venv/bin/python3 scripts/run_test.py "$$f" build/debug/protoduck.duckdb_extension; \
		fi \
	done

# Run SQL tests with release build
test_release: release
	@echo "Running SQL tests with release build..."
	@for f in test/sql/*.test; do \
		if [ -f "$$f" ]; then \
			echo "Running $$f..."; \
			.venv/bin/python3 scripts/run_test.py "$$f" build/release/protoduck.duckdb_extension; \
		fi \
	done

# Clean build artifacts
clean:
	cargo clean
	rm -rf build/

# Deep clean including venv
clean_all: clean
	rm -rf .venv/

# Format code
fmt:
	cargo fmt

# Lint code
lint:
	cargo clippy -- -D warnings

# Check compilation without building
check:
	cargo check

# Install to DuckDB extensions directory (Linux/macOS)
install: release
	@mkdir -p ~/.duckdb/extensions
	@cp build/release/protoduck.duckdb_extension ~/.duckdb/extensions/
	@echo "Installed to ~/.duckdb/extensions/protoduck.duckdb_extension"
