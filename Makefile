build-dev:
	cargo build --tests

setup-grcov:
	cargo install grcov

coverage: build-dev setup-grcov
	RUSTFLAGS="-C instrument-coverage" cargo test
	grcov . -s . --binary-path ./target/debug/ -t html --branch --ignore-not-existing -o ./target/debug/coverage/
	rm *.profraw

setup-rust-checks:
	rustup component add rustfmt
	cargo install cargo-audit
	rustup component add clippy

checks: setup-rust-checks
	cargo fmt -- --check
	cargo audit
	cargo clippy --all --all-targets --all-features -- -D warnings