#!/usr/bin/env bash
set -euo pipefail

# Install JS dependencies
bun install

# Generate SvelteKit types
bunx svelte-kit sync

# Full Rust build so dev.sh starts fast (binary is already compiled)
cargo build --manifest-path src-tauri/Cargo.toml
cargo build --manifest-path src-tauri/Cargo.toml --bin autodev-mcp
