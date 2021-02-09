all: test
.PHONY: all

test: 
	@echo TEST DEFAULT FEATURES
	@cargo test --all
	@echo TEST WITH BACKTRACE
	@cargo test --features backtrace --all
	@echo TEST NO DEFAULT FEATURES
	@cargo check --no-default-features --all
.PHONY: test

check:
	@cargo check --all
.PHONY: check
