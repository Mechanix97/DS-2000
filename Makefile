include discord.env
export

run:
	@cargo tauri dev

deps:
	@cargo install tauri-cli --version "^2.0.0" --locked
	@cd src-tauri && cd frontend && npm install
	
build-installer-windows:
	@cargo tauri bundle

clean:
	@cd src-tauri && cargo clean

lint:
	@cd src-tauri && cargo clippy --all-targets --all-features -- -D warnings

test:
	@cd src-tauri && cargo test -- --nocapture --test-threads=1

test-discord:
	@cd src-tauri && cargo test -p discord
