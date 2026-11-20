.PHONY: install install-cli install-python build test check serve mcp clean

install: install-cli install-python

install-cli:
	cargo install --path crates/bughunter-cli --force

install-python:
	pip install -e sdk/python

build:
	cargo build --workspace

check:
	cargo check --workspace

test:
	cargo test --workspace
	PYTHONPATH=sdk/python python3 -m unittest discover -s sdk/python/tests -v

serve:
	cargo run -p bughunter-cli -- serve --addr 127.0.0.1:8080

mcp:
	cargo run -p bughunter-cli -- mcp --transport stdio

clean:
	cargo clean
