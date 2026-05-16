//! Semantic parser

use tree_sitter::{Parser, Tree};

/// Errors that can occur while parsing Tera source.
#[derive(Debug)]
pub struct ParseError {
    kind: ParseErrorKind,
}

#[derive(Debug)]
enum ParseErrorKind {
    Language(tree_sitter::LanguageError),
    ParseCancelled,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            ParseErrorKind::Language(error) => {
                write!(formatter, "failed to configure the Tera parser: {error}")
            }
            ParseErrorKind::ParseCancelled => {
                formatter.write_str("tree-sitter did not produce a parse tree")
            }
        }
    }
}

impl std::error::Error for ParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            ParseErrorKind::Language(error) => Some(error),
            ParseErrorKind::ParseCancelled => None,
        }
    }
}

impl From<tree_sitter::LanguageError> for ParseError {
    fn from(error: tree_sitter::LanguageError) -> Self {
        Self {
            kind: ParseErrorKind::Language(error),
        }
    }
}

/// Parses Tera source text into a Tree-sitter syntax tree.
pub fn parse(source: &str) -> Result<Tree, ParseError> {
    let mut parser = Parser::new();
    parser.set_language(&crate::tree_sitter_tera::LANGUAGE.into())?;

    parser.parse(source, None).ok_or(ParseError {
        kind: ParseErrorKind::ParseCancelled,
    })
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn parses_output_reference() {
        let tree = parse("{{ project_name }}").expect("valid Tera should parse");

        assert!(!tree.root_node().has_error());
    }

    #[test]
    fn parses_control_flow() {
        let tree = parse("{% for item in items %}{{ item.name }}{% endfor %}")
            .expect("valid Tera should parse");

        assert!(!tree.root_node().has_error());
    }
}
