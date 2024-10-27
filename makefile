.PHONY: build

build:
	@echo "Building..."
	cd edge-flow && cargo build

test:
	@echo "Testing..."
	cd edge-flow && cargo test