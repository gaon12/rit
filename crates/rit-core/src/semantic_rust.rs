use crate::{Result, RitError};
use std::collections::BTreeMap;
use tree_sitter::Node;

/// Rust function-level semantic summary.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RustSemanticSummary {
    /// Function names present only in the new source.
    pub added_functions: Vec<String>,
    /// Function names present only in the old source.
    pub deleted_functions: Vec<String>,
    /// Function names present in both sources with changed function text.
    pub changed_functions: Vec<RustFunctionChange>,
}

/// Changed Rust function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustFunctionChange {
    /// Function name.
    pub name: String,
}

/// Summarizes added, deleted, and changed Rust functions.
pub fn summarize_rust_functions(old_source: &str, new_source: &str) -> Result<RustSemanticSummary> {
    let old_functions = rust_functions(old_source)?;
    let new_functions = rust_functions(new_source)?;

    let added_functions = new_functions
        .keys()
        .filter(|name| !old_functions.contains_key(*name))
        .cloned()
        .collect();
    let deleted_functions = old_functions
        .keys()
        .filter(|name| !new_functions.contains_key(*name))
        .cloned()
        .collect();
    let changed_functions = old_functions
        .iter()
        .filter_map(|(name, old_text)| {
            new_functions
                .get(name)
                .filter(|new_text| *new_text != old_text)
                .map(|_| RustFunctionChange { name: name.clone() })
        })
        .collect();

    Ok(RustSemanticSummary {
        added_functions,
        deleted_functions,
        changed_functions,
    })
}

fn rust_functions(source: &str) -> Result<BTreeMap<String, String>> {
    let mut parser = tree_sitter::Parser::new();
    let language = tree_sitter_rust::LANGUAGE.into();
    parser.set_language(&language).map_err(|error| {
        RitError::invalid_input(format!("Rust parser rejected language: {error}"))
    })?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| RitError::invalid_input("Rust parser did not return a syntax tree"))?;

    let mut functions = BTreeMap::new();
    collect_functions(tree.root_node(), source, &mut functions);
    Ok(functions)
}

fn collect_functions(node: Node<'_>, source: &str, functions: &mut BTreeMap<String, String>) {
    if node.kind() == "function_item"
        && let Some(name) = function_name(node, source)
    {
        functions.insert(name, source[node.byte_range()].to_owned());
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_functions(child, source, functions);
    }
}

fn function_name(node: Node<'_>, source: &str) -> Option<String> {
    node.child_by_field_name("name")?
        .utf8_text(source.as_bytes())
        .ok()
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarizes_rust_function_changes() {
        let old_source = r#"
            fn unchanged() -> i32 { 1 }
            fn changed() -> i32 { 1 }
            fn deleted() {}
        "#;
        let new_source = r#"
            fn unchanged() -> i32 { 1 }
            fn changed() -> i32 { 2 }
            fn added() {}
        "#;

        let summary =
            summarize_rust_functions(old_source, new_source).expect("Rust summary should parse");

        assert_eq!(summary.added_functions, vec!["added"]);
        assert_eq!(summary.deleted_functions, vec!["deleted"]);
        assert_eq!(
            summary.changed_functions,
            vec![RustFunctionChange {
                name: "changed".to_owned(),
            }]
        );
    }
}
