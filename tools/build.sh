#!/usr/bin/env bash
# Builds rake-lang.org into public/ from src/ and the Rake documentation, or
# with "check", confirms public/ matches a fresh build and passes the search
# and link checks. RAKE_DIR is the compiler checkout whose docs are published
# (default ../rake), RAKE_BRANCH the branch GitHub links point at (default
# main), and TREE_SITTER_RAKE_DIR the grammar checkout
# (default ../tree-sitter-rake).
set -euo pipefail

site_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if test "${RAKE_SITE_SHELL:-}" != 1; then
  exec nix develop "${site_root}" --command env RAKE_SITE_SHELL=1 bash "$0" "$@"
fi
export RAKE_DIR="${RAKE_DIR:-${site_root}/../rake}"
export RAKE_BRANCH="${RAKE_BRANCH:-main}"
tree_sitter_rake="${TREE_SITTER_RAKE_DIR:-${site_root}/../tree-sitter-rake}"
export PLAYGROUND_ASSETS="${site_root}/.build/playground"
mode="${1:-build}"

rm -rf "${PLAYGROUND_ASSETS}"
mkdir -p "${PLAYGROUND_ASSETS}/scripts"

(cd "${RAKE_DIR}" && nix develop --command dune build --profile release src/browser/playground.bc.js)
cp "${RAKE_DIR}/_build/default/src/browser/playground.bc.js" \
  "${PLAYGROUND_ASSETS}/scripts/rake-compiler.js"

npm ci --ignore-scripts \
  --prefix "${site_root}/tools/playground"
npm run build \
  --prefix "${site_root}/tools/playground"
cp "${site_root}/tools/playground/dist/playground.js" \
  "${PLAYGROUND_ASSETS}/scripts/playground.js"
cp "${site_root}/tools/playground/dist/playground-worker.js" \
  "${PLAYGROUND_ASSETS}/scripts/playground-worker.js"
cp "${site_root}/tools/playground/node_modules/web-tree-sitter/tree-sitter.wasm" \
  "${PLAYGROUND_ASSETS}/scripts/tree-sitter.wasm"

tree-sitter build --wasm -o "${PLAYGROUND_ASSETS}/scripts/tree-sitter-rake.wasm" \
  "${tree_sitter_rake}"
cp "${tree_sitter_rake}/queries/highlights.scm" \
  "${PLAYGROUND_ASSETS}/scripts/rake-highlights.scm"

cargo run --quiet --release --manifest-path "${site_root}/tools/site/Cargo.toml" -- "${mode}"
