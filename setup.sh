#!/usr/bin/env bash
set -euo pipefail

# Install JS dependencies
bun install

# Generate SvelteKit types
bunx svelte-kit sync

# Build Rust deps + generate Tauri schemas (non-interactive)
cargo check --manifest-path src-tauri/Cargo.toml
