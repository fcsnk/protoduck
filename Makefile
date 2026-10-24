.PHONY: all clean clean_all configure debug release test test_debug test_release fmt lint check install

PROJ_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))

EXTENSION_NAME=protoduck

# The stable C API (no USE_UNSTABLE_C_API): one binary loads on every DuckDB from v1.5.6 (the floor the entrypoint
# declares) through v2.0.x. The unstable ABI pins a binary to one exact DuckDB, and DuckDB 2.0 requires an unstable-ABI
# extension to export the v2 entrypoint (<name>_init_c_api_v2), which duckdb-rs only offers on its duckdb-neo API.

TARGET_DUCKDB_VERSION=v1.5.6

all: release

# Auto-init the extension-ci-tools submodule when missing so fresh clones
# (without --recursive) can still build.
ifeq ($(wildcard extension-ci-tools/makefiles/c_api_extensions/base.Makefile),)
  $(info extension-ci-tools submodule not initialized; running git submodule update --init --recursive)
  _ := $(shell git submodule update --init --recursive)
endif

include extension-ci-tools/makefiles/c_api_extensions/base.Makefile
include extension-ci-tools/makefiles/c_api_extensions/rust.Makefile

configure: venv platform extension_version

debug: build_extension_library_debug build_extension_with_metadata_debug

release: build_extension_library_release build_extension_with_metadata_release

test: test_debug

test_debug: test_extension_debug

test_release: test_extension_release

clean: clean_build clean_rust

clean_all: clean_configure clean

fmt:
	cargo fmt

lint:
	cargo clippy -- -D warnings

check:
	cargo check

install: release
	@PLATFORM=$$(cat configure/platform.txt); \
	mkdir -p ~/.duckdb/extensions/$$PLATFORM; \
	cp build/release/$(EXTENSION_NAME).duckdb_extension ~/.duckdb/extensions/$$PLATFORM/; \
	echo "Installed to ~/.duckdb/extensions/$$PLATFORM/"
