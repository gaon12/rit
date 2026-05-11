use crate::{Result, RitError};
use std::collections::BTreeMap;
use tree_sitter::Node;

/// Python function-level semantic summary.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PythonSemanticSummary {
    /// Function names present only in the new source.
    pub added_functions: Vec<String>,
    /// Function names present only in the old source.
    pub deleted_functions: Vec<String>,
    /// Function names present in both sources with changed function text.
    pub changed_functions: Vec<PythonFunctionChange>,
}

/// Changed Python function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonFunctionChange {
    /// Function name.
    pub name: String,
}

/// Summarizes added, deleted, and changed Python functions.
pub fn summarize_python_functions(
    old_source: &str,
    new_source: &str,
) -> Result<PythonSemanticSummary> {
    let old_functions = python_functions(old_source)?;
    let new_functions = python_functions(new_source)?;

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
                .map(|_| PythonFunctionChange { name: name.clone() })
        })
        .collect();

    Ok(PythonSemanticSummary {
        added_functions,
        deleted_functions,
        changed_functions,
    })
}

fn python_functions(source: &str) -> Result<BTreeMap<String, String>> {
    let mut parser = tree_sitter::Parser::new();
    let language = tree_sitter_python::LANGUAGE.into();
    parser.set_language(&language).map_err(|error| {
        RitError::invalid_input(format!("Python parser rejected language: {error}"))
    })?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| RitError::invalid_input("Python parser did not return a syntax tree"))?;

    let mut functions = BTreeMap::new();
    collect_functions(tree.root_node(), source, &mut functions);
    Ok(functions)
}

fn collect_functions(node: Node<'_>, source: &str, functions: &mut BTreeMap<String, String>) {
    if node.kind() == "function_definition"
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
    fn summarizes_python_function_changes() {
        let old_source = r#"
def unchanged():
    return 1

def changed():
    return 1

def deleted():
    pass
        "#;
        let new_source = r#"
def unchanged():
    return 1

def changed():
    return 2

def added():
    pass
        "#;

        let summary = summarize_python_functions(old_source, new_source)
            .expect("Python summary should parse");

        assert_eq!(summary.added_functions, vec!["added"]);
        assert_eq!(summary.deleted_functions, vec!["deleted"]);
        assert_eq!(
            summary.changed_functions,
            vec![PythonFunctionChange {
                name: "changed".to_owned(),
            }]
        );
    }
}
