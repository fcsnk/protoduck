.PHONY: all clean clean_all configure debug release test test_debug test_release fmt lint check install

PROJ_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))

EXTENSION_NAME=protoduck

# Required: duckdb-rs vscalar feature uses unstable C API
USE_UNSTABLE_C_API=1

TARGET_DUCKDB_VERSION=v1.5.2

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
