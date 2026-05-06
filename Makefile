build:
	cargo build --workspace
	npm run build

test:
	cargo test --workspace

lint:
	cargo clippy --workspace -- -D warnings -D clippy::unwrap_used

dev-ui:
	npm run dev --workspace=packages/web-ui
