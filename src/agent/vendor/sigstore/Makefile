.PHONY: build
build:
	cargo build --release

.PHONY: fmt
fmt:
	cargo fmt --all -- --check

.PHONY: lint
lint:
	cargo clippy -- -D warnings

.PHONY: doc
doc:
	cargo doc

.PHONY: test
test: fmt lint doc
	cargo test --workspace

.PHONY: clean
clean:
	cargo clean

.PHONY: coverage
coverage:
	cargo tarpaulin -o Html
