x86_64 backends
run one:
```
cargo run --features=vulkan --bin=main
cargo run --features=gl --bin=main
```

wasm32 backend
deps:
```
npm install http-server -g
cargo install wasm-bindgen-cli
```
run:
```
cargo build --target=wasm32-unknown-unknown --features=gl --bin=main
wasm-bindgen target/wasm32-unknown-unknown/debug/main.wasm --out-dir pkg --web
cd pkg
http-server
```