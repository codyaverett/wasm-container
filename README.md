# WASM Container Runtime

A WebAssembly-based container runtime that can execute Docker-compatible containers within WASM environments.

## Features

- **WASM Runtime**: Built on top of Wasmtime for secure and efficient execution
- **Container Filesystem**: Isolated filesystem layers with volume mounting support
- **Networking**: Port forwarding and network isolation
- **Image Support**: Basic OCI image format compatibility
- **Docker-like CLI**: Familiar command interface

## Installation

```bash
git clone https://github.com/codyaverett/wasm-container.git
cd wasm-container
cargo build --release
```

## Usage

### Run a Container

```bash
# Basic container execution
wasm-container run hello-world

# With custom command
wasm-container run ubuntu:latest --command /bin/bash

# With environment variables
wasm-container run myapp:latest --env PORT=8080 --env DEBUG=true

# With working directory
wasm-container run myapp:latest --workdir /app
```

### Pull an Image

```bash
wasm-container pull ubuntu:latest
```

### List Containers

```bash
# List running containers
wasm-container list

# List all containers (including stopped)
wasm-container list --all
```

### Stop a Container

```bash
wasm-container stop <container-id>
```

## Architecture

The WASM Container Runtime consists of several key components:

- **Runtime**: Core WASM execution engine using Wasmtime
- **Container**: Container lifecycle management and configuration
- **Filesystem**: Layered filesystem with volume support
- **Network**: Network isolation and port forwarding
- **Image**: OCI image parsing and caching

## Building Containers for WASM

To create containers compatible with this runtime, you need to compile your application to WebAssembly:

```bash
# Example: Building a Rust application for WASM
cargo build --target wasm32-wasi --release
```

Then create a simple Dockerfile-like manifest or use the standard OCI format.

## Limitations

This is a proof-of-concept implementation with the following limitations:

- Limited OCI image format support
- Basic networking implementation
- No orchestration features
- Simplified security model
- Demo WASM binaries only

## Development

```bash
# Run tests
cargo test

# Run with debug logging
RUST_LOG=debug cargo run -- run hello-world

# Format code
cargo fmt

# Run linter
cargo clippy
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

## License

MIT License - see LICENSE file for details.
