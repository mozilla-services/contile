build:
	cargo build

setup-all: build setup-coverage-tools setup-rust-checks

setup-coverage-tools:
	cargo +stable install cargo-llvm-cov --locked

coverage: setup-coverage-tools
	cargo llvm-cov --open

setup-rust-checks:
	rustup component add rustfmt
	cargo install cargo-audit
	rustup component add clippy

checks: setup-rust-checks
	cargo fmt -- --check
	cargo audit
	cargo clippy --all --all-targets --all-features -- -D warnings

install-doc-dependencies:
	cargo install mdbook && cargo install mdbook-mermaid

build-docs: 
	mdbook-mermaid install && mdbook clean && mdbook build

doc-preview: build-docs
	mdbook serve --open