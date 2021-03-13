x86_64 backends
run one:
```
cargo run --features=vulkan --bin=main
cargo run --features=gl --bin=main
```

wasm32 backend
deps:
```
npm install light-server -g
cargo install wasm-bindgen-cli
rustup target add wasm32-unknown-unknown
```
build:
```
cargo build --target=wasm32-unknown-unknown --features=gl --bin=main
wasm-bindgen ../../target/wasm32-unknown-unknown/debug/main.wasm --out-dir pkg --web
```
run:
```
cd pkg
light-server
```
lazy:
```
./run-wasm.sh
```