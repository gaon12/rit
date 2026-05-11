use super::{
    SemanticDiffFile, SemanticFileCategory, WordDiffOperation, semantic_report_from_paths,
    word_diff,
};

#[test]
fn word_diff_reports_insertions_and_deletions() {
    let diff = word_diff("fn add(a: i32) -> i32", "fn sum(a: i64) -> i64");

    assert_eq!(
        diff.operations,
        vec![
            WordDiffOperation::Equal("fn".to_owned()),
            WordDiffOperation::Delete("add".to_owned()),
            WordDiffOperation::Insert("sum".to_owned()),
            WordDiffOperation::Equal("(".to_owned()),
            WordDiffOperation::Equal("a".to_owned()),
            WordDiffOperation::Equal(":".to_owned()),
            WordDiffOperation::Delete("i32".to_owned()),
            WordDiffOperation::Insert("i64".to_owned()),
            WordDiffOperation::Equal(")".to_owned()),
            WordDiffOperation::Equal("-".to_owned()),
            WordDiffOperation::Equal(">".to_owned()),
            WordDiffOperation::Delete("i32".to_owned()),
            WordDiffOperation::Insert("i64".to_owned()),
        ]
    );
}

#[test]
fn unchanged_word_diff_is_empty() {
    assert!(word_diff("let value = 1;", "let value = 1;").is_empty());
}

#[test]
fn semantic_report_distinguishes_code_tests_and_docs() {
    let report = semantic_report_from_paths([
        "src/lib.rs",
        "tests/compat.rs",
        "docs/readme.md",
        "assets/logo.png",
    ]);

    assert_eq!(
        report.files,
        vec![
            SemanticDiffFile {
                path: "src/lib.rs".to_owned(),
                category: SemanticFileCategory::Code,
            },
            SemanticDiffFile {
                path: "tests/compat.rs".to_owned(),
                category: SemanticFileCategory::Tests,
            },
            SemanticDiffFile {
                path: "docs/readme.md".to_owned(),
                category: SemanticFileCategory::Docs,
            },
            SemanticDiffFile {
                path: "assets/logo.png".to_owned(),
                category: SemanticFileCategory::Other,
            },
        ]
    );
    assert!(!report.is_code_only());
}

#[cfg(feature = "semantic-json")]
#[test]
fn semantic_report_serializes_to_json() {
    let report = semantic_report_from_paths(["src/lib.rs"]);
    let json = report.to_json_string().expect("json should serialize");

    assert!(json.contains("\"path\": \"src/lib.rs\""));
    assert!(json.contains("\"category\": \"code\""));
}
