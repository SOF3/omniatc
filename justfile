fmt:
	cargo +nightly fmt
maps:
	cargo run -p omniatc-maps build-assets
fix:
	cargo clippy --fix --allow-staged --all --tests
run:
	RUST_BACKTRACE=1 \
		RUST_LOG=warn,omniatc=debug \
		cargo run -p omniatc-client -F dev
run-scenario which:
	RUST_BACKTRACE=1 \
		RUST_LOG=warn,omniatc=debug \
		cargo run -p omniatc-client -F dev -- \
		--default-scenario {{which}}

precommit:
	cargo +nightly fmt -- --check
	cargo clippy --all --tests -- \
		-D warnings \
		-D clippy::dbg_macro \
		-D dead_code \
		-D unused_variables \
		-D unused_imports
	cargo test --all

docker-test:
	docker build -f tests/Dockerfile .
