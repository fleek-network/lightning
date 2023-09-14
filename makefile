.PHONY: fmt
fmt:
	cargo fmt --all

.PHONY: lint
lint:
	cargo clippy --all-features --all-targets --timings -- -Dclippy::all -Dwarnings
