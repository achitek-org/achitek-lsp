//! Vendored Tera grammar binding.

use tree_sitter_language::LanguageFn;

unsafe extern "C" {
    fn tree_sitter_tera() -> *const ();
}

pub(crate) const LANGUAGE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_tera) };
