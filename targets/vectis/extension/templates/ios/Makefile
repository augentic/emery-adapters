.PHONY: all build clean typegen package xcode sim-build sim-install sim-launch sim-run run sim-app-path

SHARED_DIR := ../shared

all: build

build: typegen package xcode

# Phase 1: generate SharedTypes (domain types as a Swift package)
typegen:
	@echo "Generating SharedTypes..."
	@RUST_LOG=info RUSTFLAGS="-D warnings" cargo run --manifest-path $(SHARED_DIR)/Cargo.toml \
		--bin codegen --features codegen,facet_typegen -- \
		--language swift --output-dir generated

# Phase 2: build Shared (UniFFI bindings as a Swift package via cargo-swift)
package:
	@echo "Building Shared Swift package..."
	@cd $(SHARED_DIR) && \
		RUSTFLAGS="-D warnings" cargo swift package --name Shared --platforms ios \
			--lib-type static --features uniffi && \
		rm -rf ../iOS/generated/Shared && \
		mkdir -p ../iOS/generated/Shared && \
		cp -r Shared/* ../iOS/generated/Shared/ && \
		rm -rf Shared

# Phase 3: generate Xcode project from project.yml
xcode:
	@echo "Generating Xcode project..."
	@xcodegen

# Build the iOS app for the simulator (destination lives in .vectis/sim-build.sh)
sim-build:
	@bash "$(dir $(lastword $(MAKEFILE_LIST)))/.vectis/sim-build.sh"

sim-install:
	@bash "$(dir $(lastword $(MAKEFILE_LIST)))/.vectis/sim-dev.sh" install

sim-launch:
	@bash "$(dir $(lastword $(MAKEFILE_LIST)))/.vectis/sim-dev.sh" launch

sim-run:
	@bash "$(dir $(lastword $(MAKEFILE_LIST)))/.vectis/sim-dev.sh" run

run: sim-run

sim-app-path:
	@bash "$(dir $(lastword $(MAKEFILE_LIST)))/.vectis/sim-dev.sh" app-path

clean:
	@rm -rf generated/ DerivedData/ *.xcodeproj
