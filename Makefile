.DEFAULT_GOAL := help

.PHONY: help build release run tui test test-render lint lint-md fmt fmt-check check audit install uninstall completions clean

help: ## Show this help
	@grep -E '^[a-zA-Z0-9_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-14s\033[0m %s\n", $$1, $$2}'

build: ## Build the debug binary
	cargo build --workspace

release: ## Build the optimized release binary
	cargo build --workspace --release

run: ## Run the debug binary (use ARGS="snippet.swift --line-numbers")
	cargo run -p snapcode -- $(ARGS)

tui: ## Open the TUI on a file (use FILE=snippet.swift)
	cargo run -p snapcode -- tui $(FILE)

test: ## Run the test suite
	cargo test --workspace

# The pixel-level rendering tests (tests/render.rs) and the PNG/SVG parity
# check (tests/svg_parity.rs). Already covered by `make test`; this is just a
# faster way to re-run the ones that catch visual regressions.
test-render: ## Run only the rendering and SVG-parity tests
	cargo test -p snapcode-core --test render --test svg_parity

lint: ## Run clippy with warnings denied
	cargo clippy --workspace --all-targets -- -D warnings

lint-md: ## Lint markdown files (markdownlint-cli2, via npx)
	npx --yes markdownlint-cli2@0.23.2 "**/*.md"

fmt: ## Apply rustfmt
	cargo fmt --all

fmt-check: ## Check formatting without modifying files
	cargo fmt --all --check

check: fmt-check lint lint-md test ## Run fmt-check, lint, lint-md, and test: the full pre-commit gate

audit: ## Check dependencies for known security advisories (cargo-audit)
	cargo audit

install: ## Install the snapcode binary via cargo (~/.cargo/bin)
	cargo install --path crates/snapcode-cli --locked --force

uninstall: ## Remove the installed snapcode binary
	cargo uninstall snapcode

completions: ## Write shell completions to target/completions/
	@mkdir -p target/completions
	@for shell in bash zsh fish; do \
		cargo run -q -p snapcode -- completions $$shell > target/completions/snapcode.$$shell; \
	done
	@echo "wrote target/completions/snapcode.{bash,zsh,fish}"

clean: ## Remove build artifacts
	cargo clean
