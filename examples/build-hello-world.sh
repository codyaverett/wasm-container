#!/bin/bash

# Build the hello-world example as a WASM binary
echo "Building hello-world example for WASM..."

# Ensure we have the wasm32-wasi target
rustup target add wasm32-wasi

# Build the example
cargo build --example hello-world --target wasm32-wasi --release

# Copy the result to a more convenient location
cp target/wasm32-wasi/release/examples/hello-world.wasm ./hello-world.wasm

echo "Built hello-world.wasm successfully!"
echo "You can now test it with: wasm-container run hello-world"