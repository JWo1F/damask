//! The Tree-sitter grammar for Damask templates.
//!
//! The grammar is developed and released as its own repository — see
//! `grammar/README.md` for the revision this copy was taken from. That
//! repository ships no Rust bindings (editors compile `parser.c` themselves),
//! so this crate is the binding: it compiles the committed parser and hands
//! back the [`LanguageFn`] that `tree-sitter` wants.
//!
//! The highlight and injection queries are *not* here. They live with the Zed
//! extension, in `editors/zed/languages/damask/`, and the website reads them
//! from there — one set of queries decides what a `.dmk` snippet looks like,
//! whether it is being read in an editor or on the website.

use tree_sitter_language::LanguageFn;

// SAFETY: `grammar/src/parser.c`, compiled by `build.rs`, defines this symbol
// and returns a pointer to a `TSLanguage` with static storage duration. The
// signature matches the one Tree-sitter's code generator emits.
unsafe extern "C" {
    fn tree_sitter_damask() -> *const ();
}

/// The Damask language, for `tree_sitter::Parser::set_language`.
pub const LANGUAGE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_damask) };

#[cfg(test)]
mod tests {
    #[test]
    fn the_grammar_loads() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&super::LANGUAGE.into())
            .expect("load the Damask grammar");

        let tree = parser.parse("<p>{ self.name }</p>", None).expect("parse");
        assert!(!tree.root_node().has_error());
    }
}
