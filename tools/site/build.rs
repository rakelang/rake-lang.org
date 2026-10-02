// Compiles the Rake Tree-sitter grammar from its sibling checkout, so the
// site's highlighting is the grammar's own and has no second definition.
use std::path::PathBuf;

fn main() {
    let grammar = std::env::var("TREE_SITTER_RAKE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../tree-sitter-rake")
        });
    let source = grammar.join("src");
    cc::Build::new()
        .include(&source)
        .file(source.join("parser.c"))
        .file(source.join("scanner.c"))
        .warnings(false)
        .compile("tree-sitter-rake");
    println!(
        "cargo:rerun-if-changed={}",
        source.join("parser.c").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        source.join("scanner.c").display()
    );
    println!("cargo:rerun-if-env-changed=TREE_SITTER_RAKE_DIR");
    println!(
        "cargo:rustc-env=TREE_SITTER_RAKE_QUERIES={}",
        grammar.join("queries").display()
    );
}
