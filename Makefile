# Sibling augentic/specify checkout — owns the canonical dev-loop
# orchestration (scripts/dev.sh); override with SPECIFY_FRAMEWORK=<path>.
SPECIFY_FRAMEWORK ?= $(CURDIR)/../specify
DEV := SPECIFY_ADAPTERS="$(CURDIR)" SPECIFY_FRAMEWORK="$(SPECIFY_FRAMEWORK)" \
	bash "$(SPECIFY_FRAMEWORK)/scripts/dev.sh"

.PHONY: dev-doctor dev-check dev-run dev-live dev-full

# Validate sibling layout, toolchain, WASI target, and cursor-agent.
# LIVE=1 adds a command-mode credential probe (one real model call).
dev-doctor:
	@$(DEV) doctor $(if $(LIVE),--live,)

# Fastest model-free rung: this repo's adapter native tests (scope with
# ADAPTER=<name>) plus the specify checkout's native harness suite.
dev-check:
	@$(DEV) check $(ADAPTER)

# Run specify-dev against any consumer project without changing
# directory: make dev-run PROJECT=/path/to/project ARGS='plan status'.
dev-run:
	@$(DEV) run "$(PROJECT)" $(ARGS)

# One deliberate live-model run. Bare: the native-shim guest execute
# loop. ADAPTER=<name> [SCENARIO=<live test>]: exactly one adapter live
# eval scenario from evals/live.rs (prose overlay on once artifacts
# exist).
dev-live:
	@$(DEV) live $(ADAPTER) $(SCENARIO)

# The explicit outer gate: doctor --live, deterministic checks,
# composed WASM/WIT coverage, and the composed guest execute loop.
dev-full:
	@$(DEV) full

# dynamically target Makefile.toml
.PHONY: %
%:
	@cargo make $@
