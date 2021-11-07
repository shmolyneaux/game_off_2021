.PHONY: release
release:
	cargo build --release --target wasm32-unknown-unknown
	rm -rf release
	mkdir release
	cp target/wasm32-unknown-unknown/release/macroquad-shimlang.wasm release/
	cp index.html release/
	cp -r assets release/assets
	zip -r release.zip release
