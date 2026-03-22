fmt:
	cargo +nightly fmt
maps:
	cargo run -p omniatc-maps build-assets
fix:
	cargo clippy --fix --allow-staged --all --tests -- -D warnings -D clippy::dbg_macro -D dead_code -D unused_variables -D unused_imports
run:
	RUST_BACKTRACE=1 \
		RUST_LOG=warn,omniatc=debug \
		cargo run -p omniatc-client -F dev
run-scenario which:
	RUST_BACKTRACE=1 \
		RUST_LOG=warn,omniatc=debug \
		cargo run -p omniatc-client -F dev -- \
		--default-scenario {{which}}

client-tests-clean-baseline:
	if [ -d tests/client/screenshots ]; then \
		find tests/client/screenshots -type f -delete; \
	fi
client-tests:
	docker build --build-arg OMNIATC_UID=${UID} -t omniatc-client-tests -f tests/Dockerfile .
	docker rm -f omniatc-client-tests || true
	[ -d tests/client/screenshots ] || mkdir tests/client/screenshots
	docker run \
		--name omniatc-client-tests \
		-v ./tests/client/screenshots:/var/screenshots \
		-e SCREENSHOTS_DIR=/var/screenshots \
		-e RUST_LOG=error,omniatc=debug \
		omniatc-client-tests

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
