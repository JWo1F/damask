//! Compiles the vendored Tree-sitter parser.

fn main() {
    let src = std::path::Path::new("grammar/src");

    println!("cargo:rerun-if-changed={}", src.join("parser.c").display());

    cc::Build::new()
        // `parser.c` includes its headers as `tree_sitter/parser.h`, so the
        // directory holding that folder is what goes on the include path.
        .include(src)
        .file(src.join("parser.c"))
        // A generated parser is a table plus a switch several thousand cases
        // wide. Compilers warn about it, the warnings are not actionable
        // against a file nobody edits by hand, and CI builds with `-D warnings`.
        .warnings(false)
        .compile("tree-sitter-damask");
}
