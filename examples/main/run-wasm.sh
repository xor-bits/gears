#!/bin/bash
script_path=$(dirname "$0")
current_path=$(pwd)

cd $script_path
echo $(pwd)

cargo build --target=wasm32-unknown-unknown --features=gl --example=main
wasm-bindgen target/wasm32-unknown-unknown/debug/examples/main.wasm --out-dir pkg --web
cd pkg
light-server --serve . --open

cd $current_path