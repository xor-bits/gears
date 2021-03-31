#!/bin/bash
script_path=$(dirname "$0")
current_path=$(pwd)

cd $script_path

cargo build --target=wasm32-unknown-unknown --features=gl --bin=main
wasm-bindgen ../../target/wasm32-unknown-unknown/debug/main.wasm --out-dir pkg --web
cd pkg
light-server --serve .

cd $current_path