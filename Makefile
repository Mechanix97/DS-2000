include secrets\discord.env
export

run:
	@cargo tauri dev

deps:
	@cd src-tauri && cd frontend && npm install
	
build_installer_windows:
	@cargo tauri bundle
