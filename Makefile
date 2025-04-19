include discord.env
export

run:
	@cargo tauri dev

deps:
	@cargo install tauri-cli --version "^2.0.0" --locked
	@cd src-tauri && cd frontend && npm install
	
build_installer_windows:
	@cargo tauri bundle
