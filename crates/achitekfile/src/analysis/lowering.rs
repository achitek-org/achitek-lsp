//! Lowers the Tree-sitter syntax tree into the forgiving Achitekfile model.
//!
//! The parser layer preserves source syntax exactly as Tree-sitter sees it.
//! This module performs the next analysis step: it walks named syntax nodes,
//! extracts Achitekfile concepts, decodes literal values, and stores source
//! ranges on recovered values. It intentionally skips malformed pieces instead
//! of reporting diagnostics directly; diagnostics are collected in the
//! diagnostics pass so model recovery and validation stay separate.
//!
//! Lowering is a crate-internal implementation detail. Public consumers should
//! use [`crate::analyze`] rather than constructing a model from a tree.

use crate::model::{
    AchitekFile, Blueprint, ComparisonOperator, Dependency, Prompt, PromptType, Spanned,
    Validation, Value,
};
use achitek_source::{named_children, text, text_range_for_node};
use tree_sitter::{Node, Tree};

impl AchitekFile {
    /// Creates a recovering Achitekfile model from a parsed Tree-sitter tree.
    ///
    /// The resulting model keeps optional fields optional and preserves source
    /// ranges for recovered declarations. Invalid or incomplete syntax is
    /// tolerated here; the diagnostics pass is responsible for explaining why
    /// skipped or missing pieces are invalid.
    pub(crate) fn from_tree(tree: &Tree, source: &str) -> Self {
        let root = tree.root_node();
        let mut cursor = root.walk();
        let mut blueprint = Blueprint::default();
        let mut prompts = Vec::new();

        for child in root.named_children(&mut cursor) {
            match child.kind() {
                "blueprint_block" => {
                    blueprint = parse_blueprint(child, source);
                }
                "prompt_block" => {
                    if let Some(prompt) = parse_prompt(child, source) {
                        prompts.push(prompt);
                    }
                }
                _ => {}
            }
        }

        AchitekFile::new(blueprint, prompts)
    }
}

/// Lowers a `blueprint_block` syntax node into recovered blueprint metadata.
///
/// Unknown attributes and malformed values are ignored here so diagnostics can
/// be produced from the syntax tree with better source locations.
fn parse_blueprint(node: Node<'_>, source: &str) -> Blueprint {
    let mut blueprint = Blueprint {
        range: Some(text_range_for_node(node)),
        ..Blueprint::default()
    };
    for child in named_children(node) {
        if child.kind() != "blueprint_attribute" {
            continue;
        }

        let Some(key_node) = child.child_by_field_name("key") else {
            continue;
        };

        let Some(value_node) = child.child_by_field_name("value") else {
            continue;
        };

        let key = text(key_node, source);
        let Some(value) = parse_string_literal(value_node, source) else {
            continue;
        };
        let spanned = Spanned {
            value,
            range: text_range_for_node(child),
        };

        match key {
            "version" => blueprint.version = Some(spanned),
            "name" => blueprint.name = Some(spanned),
            "description" => blueprint.description = Some(spanned),
            "author" => blueprint.author = Some(spanned),
            "min_achitek_version" => blueprint.min_achitek_version = Some(spanned),
            _ => {}
        }
    }

    blueprint
}

/// Lowers a `prompt_block` syntax node into a recovered prompt declaration.
///
/// Returns `None` when the prompt name cannot be recovered, because the prompt
/// cannot be identified meaningfully without it. Other missing pieces remain
/// optional on the recovered model.
fn parse_prompt(node: Node<'_>, source: &str) -> Option<Spanned<Prompt>> {
    let name_node = node.child_by_field_name("name")?;
    let name = parse_string_literal(name_node, source)?;
    let mut choices: Vec<Value> = Vec::new();
    let mut choices_declared = false;
    let mut prompt_type = None;
    let mut help = None;
    let mut default = None;
    let mut required = None;
    let mut depends_on = None;
    let mut validation = Validation::default();

    for child in named_children(node) {
        match child.kind() {
            "question_attribute" => {
                let Some(attribute) = named_children(child).next() else {
                    continue;
                };
                let Some(value_node) = attribute.child_by_field_name("value") else {
                    continue;
                };

                match attribute.kind() {
                    "type_attribute" => prompt_type = parse_prompt_type(value_node, source),
                    "help_attribute" => help = parse_string_literal(value_node, source),
                    "choices_attribute" => {
                        choices_declared = true;
                        choices = parse_array(value_node, source);
                    }
                    "default_attribute" => default = parse_value(value_node, source),
                    "required_attribute" => required = parse_bool(value_node, source),
                    "depends_on_attribute" => depends_on = parse_dependency(value_node, source),
                    _ => {}
                }
            }
            "validate_block" => parse_validation(child, source, &mut validation),
            _ => {}
        }
    }

    Some(Spanned {
        value: Prompt {
            name,
            prompt_type,
            help,
            choices,
            choices_declared,
            default,
            required,
            depends_on,
            validation,
        },
        range: text_range_for_node(node),
    })
}

/// Lowers validation attributes into an existing recovered validation model.
///
/// This mutates the prompt's validation state because validation attributes are
/// optional and each recognized attribute fills one field on the model.
fn parse_validation(node: Node<'_>, source: &str, validation: &mut Validation) {
    for item in named_children(node).filter(|node| node.kind() == "validate_attribute") {
        let Some(attribute) = named_children(item).next() else {
            continue;
        };
        let Some(value_node) = attribute.child_by_field_name("value") else {
            continue;
        };

        match attribute.kind() {
            "regex_attribute" => validation.regex = parse_string_literal(value_node, source),
            "min_length_attribute" => validation.min_length = parse_integer(value_node, source),
            "max_length_attribute" => validation.max_length = parse_integer(value_node, source),
            "min_selections_attribute" => {
                validation.min_selections = parse_integer(value_node, source)
            }
            "max_selections_attribute" => {
                validation.max_selections = parse_integer(value_node, source)
            }
            _ => {}
        }
    }
}

/// Converts a prompt type syntax node into a known prompt type.
fn parse_prompt_type(node: Node<'_>, source: &str) -> Option<PromptType> {
    match text(node, source) {
        "string" => Some(PromptType::String),
        "paragraph" => Some(PromptType::Paragraph),
        "bool" => Some(PromptType::Bool),
        "select" => Some(PromptType::Select),
        "multiselect" => Some(PromptType::MultiSelect),
        _ => None,
    }
}

/// Decodes an Achitekfile string literal into its model value.
///
/// Unsupported escape sequences return `None`; diagnostics for the underlying
/// invalid literal are emitted by the diagnostics pass.
fn parse_string_literal(node: Node<'_>, source: &str) -> Option<String> {
    let text = text(node, source);
    let without_open = text.strip_prefix('"')?;
    let inner = without_open.strip_suffix('"')?;

    let mut parsed = String::new();
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            parsed.push(ch);
            continue;
        }

        match chars.next()? {
            'n' => parsed.push('\n'),
            't' => parsed.push('\t'),
            'r' => parsed.push('\r'),
            '"' => parsed.push('"'),
            '\\' => parsed.push('\\'),
            _ => return None,
        }
    }

    Some(parsed)
}

/// Lowers an array syntax node into recovered model values.
fn parse_array(node: Node<'_>, source: &str) -> Vec<Value> {
    let Some(value_list) = named_children(node).find(|node| node.kind() == "value_list") else {
        return Vec::new();
    };

    named_children(value_list)
        .filter(|node| node.kind() == "value")
        .filter_map(|node| parse_value(node, source))
        .collect()
}

/// Lowers a literal syntax node into a model value.
fn parse_value(node: Node<'_>, source: &str) -> Option<Value> {
    let inner = if node.kind() == "value" || node.kind() == "literal_value" {
        named_children(node).next()?
    } else {
        node
    };

    match inner.kind() {
        "string_literal" => parse_string_literal(inner, source).map(Value::String),
        "boolean" => match text(inner, source) {
            "true" => Some(Value::Bool(true)),
            "false" => Some(Value::Bool(false)),
            _ => None,
        },
        "integer" => text(inner, source).parse::<u64>().ok().map(Value::Integer),
        "identifier" => Some(Value::Identifier(text(inner, source).to_owned())),
        "array" => Some(Value::Array(parse_array(inner, source))),
        _ => None,
    }
}

/// Converts a boolean syntax node into a Rust boolean.
fn parse_bool(node: Node<'_>, source: &str) -> Option<bool> {
    match text(node, source) {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// Converts an integer syntax node into an unsigned integer.
fn parse_integer(node: Node<'_>, source: &str) -> Option<u64> {
    text(node, source).parse::<u64>().ok()
}

/// Lowers a dependency expression into the recovered dependency model.
///
/// Unsupported method or combinator names return `None`; the diagnostics pass
/// reports those source violations separately.
fn parse_dependency(node: Node<'_>, source: &str) -> Option<Dependency> {
    let inner = if node.kind() == "dependency_expr" {
        named_children(node).next()?
    } else {
        node
    };

    match inner.kind() {
        "simple_dependency" => {
            let reference = inner.child_by_field_name("reference")?;
            Some(Dependency::Reference(text(reference, source).to_owned()))
        }
        "comparison_dependency" => {
            let left = inner.child_by_field_name("left")?;
            let right = inner.child_by_field_name("right")?;
            Some(Dependency::Comparison {
                left: text(left, source).to_owned(),
                operator: parse_comparison_operator(inner, source)?,
                right: parse_value(right, source)?,
            })
        }
        "method_call_dependency" => {
            let receiver = inner.child_by_field_name("receiver")?;
            let method = inner.child_by_field_name("method")?;
            let argument = inner.child_by_field_name("argument")?;

            if text(method, source) != "contains" {
                return None;
            }

            Some(Dependency::Contains {
                receiver: text(receiver, source).to_owned(),
                argument: parse_value(argument, source)?,
            })
        }
        "combinator_dependency" => {
            let name = inner.child_by_field_name("name")?;
            let arguments = inner.child_by_field_name("arguments")?;
            let dependencies = named_children(arguments)
                .filter(|node| node.kind() == "dependency_expr")
                .filter_map(|node| parse_dependency(node, source))
                .collect::<Vec<_>>();

            match text(name, source) {
                "all" => Some(Dependency::All(dependencies)),
                "any" => Some(Dependency::Any(dependencies)),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Extracts the comparison operator from a comparison dependency node.
fn parse_comparison_operator(node: Node<'_>, source: &str) -> Option<ComparisonOperator> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match text(child, source) {
            "==" => return Some(ComparisonOperator::Equal),
            "!=" => return Some(ComparisonOperator::NotEqual),
            _ => {}
        }
    }

    None
}
