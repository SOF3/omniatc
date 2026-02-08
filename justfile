fmt:
	cargo +nightly fmt
maps:
	cargo run -p omniatc-maps build-assets
fix:
	cargo clippy --fix --allow-staged -F precommit-checks  --all --tests
run:
	RUST_BACKTRACE=1 \
		RUST_LOG=warn,omniatc=debug \
		cargo run -p omniatc-client -F dev
precommit:
	cargo +nightly fmt -- --check
	cargo clippy --all --tests -F precommit-checks -- -D warnings
	cargo test --all
