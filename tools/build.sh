#!/usr/bin/env bash
# Builds rake-lang.org into public/ from src/ and the Rake documentation, or
# with "check", confirms public/ matches a fresh build and passes the search
# and link checks. RAKE_DIR is the compiler checkout whose docs are published
# (default ../rake-wasm-simd128), RAKE_BRANCH the branch GitHub links point
# at (default wasm-simd128), and TREE_SITTER_RAKE_DIR the grammar checkout
# (default ../tree-sitter-rake).
set -euo pipefail

site_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export RAKE_DIR="${RAKE_DIR:-${site_root}/../rake-wasm-simd128}"
mode="${1:-build}"

nix shell nixpkgs#cargo nixpkgs#rustc nixpkgs#gcc --command \
  cargo run --quiet --release --manifest-path "${site_root}/tools/site/Cargo.toml" -- "${mode}"
