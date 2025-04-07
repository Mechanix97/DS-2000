
run:
	cargo tauri dev

deps:
	@cd src-tauri && cd frontend && npm install
	
build_installer:
	cargo tauri bundle
