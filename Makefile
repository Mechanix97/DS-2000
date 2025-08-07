include discord.env
export

run:
	@set RUST_LOG=info,tauri=info&&cargo tauri dev

run-debug:
	@set RUST_LOG=debug,tauri=debug&& cargo tauri dev

deps:
	@cargo install tauri-cli --version "^2.0.0" --locked
	@cd src-tauri && cd frontend && npm install
	
build-installer-windows:
	@cd src-tauri && cargo tauri build && cargo tauri bundle

clean:
	@cd src-tauri && cargo clean

lint:
	@cd src-tauri && cargo clippy --all-targets --all-features -- -D warnings

test:
	@cd src-tauri && cargo test -- --nocapture --test-threads=1

test-discord:
	@cd src-tauri && cargo test -p discord
