/// Word-level diff result for higher-level semantic summaries.
#[cfg_attr(feature = "semantic-json", derive(serde::Serialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WordDiff {
    /// Ordered word operations.
    pub operations: Vec<WordDiffOperation>,
}

impl WordDiff {
    /// Returns true when there are no insertions or deletions.
    pub fn is_empty(&self) -> bool {
        self.operations
            .iter()
            .all(|operation| matches!(operation, WordDiffOperation::Equal(_)))
    }
}

/// One word-level diff operation.
#[cfg_attr(feature = "semantic-json", derive(serde::Serialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "semantic-json", serde(tag = "kind", content = "token"))]
pub enum WordDiffOperation {
    /// Unchanged token.
    Equal(String),
    /// Token present only in the old text.
    Delete(String),
    /// Token present only in the new text.
    Insert(String),
}

/// Structured semantic diff report suitable for JSON output.
#[cfg_attr(feature = "semantic-json", derive(serde::Serialize))]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SemanticDiffReport {
    /// Changed files in stable caller-provided order.
    pub files: Vec<SemanticDiffFile>,
}

impl SemanticDiffReport {
    /// Returns true when every changed file is classified as code.
    pub fn is_code_only(&self) -> bool {
        !self.files.is_empty()
            && self
                .files
                .iter()
                .all(|file| file.category == SemanticFileCategory::Code)
    }

    /// Serializes the report to JSON when the `semantic-json` feature is enabled.
    #[cfg(feature = "semantic-json")]
    pub fn to_json_string(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

/// One file in a semantic diff report.
#[cfg_attr(feature = "semantic-json", derive(serde::Serialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticDiffFile {
    /// Repository-relative path.
    pub path: String,
    /// Coarse category used by automation and review summaries.
    pub category: SemanticFileCategory,
}

/// Coarse file category for semantic summaries.
#[cfg_attr(feature = "semantic-json", derive(serde::Serialize))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "semantic-json", serde(rename_all = "snake_case"))]
pub enum SemanticFileCategory {
    /// Source code or configuration treated as code.
    Code,
    /// Test-only paths.
    Tests,
    /// Documentation paths and common prose formats.
    Docs,
    /// Anything not classified yet.
    Other,
}

/// Builds a semantic report from changed paths.
pub fn semantic_report_from_paths(
    paths: impl IntoIterator<Item = impl Into<String>>,
) -> SemanticDiffReport {
    SemanticDiffReport {
        files: paths
            .into_iter()
            .map(|path| {
                let path = path.into();
                let category = classify_semantic_path(&path);
                SemanticDiffFile { path, category }
            })
            .collect(),
    }
}

/// Classifies a path for semantic summaries.
pub fn classify_semantic_path(path: &str) -> SemanticFileCategory {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    if normalized.starts_with("test/")
        || normalized.starts_with("tests/")
        || normalized.contains("/tests/")
        || normalized.ends_with("_test.rs")
        || normalized.ends_with(".test.ts")
        || normalized.ends_with("_test.py")
    {
        SemanticFileCategory::Tests
    } else if normalized.starts_with("docs/")
        || normalized.ends_with(".md")
        || normalized.ends_with(".markdown")
        || normalized.ends_with(".rst")
        || normalized.ends_with(".txt")
    {
        SemanticFileCategory::Docs
    } else if normalized.ends_with(".rs")
        || normalized.ends_with(".ts")
        || normalized.ends_with(".tsx")
        || normalized.ends_with(".py")
    {
        SemanticFileCategory::Code
    } else {
        SemanticFileCategory::Other
    }
}

/// Computes a small, stable word-level diff.
pub fn word_diff(old_text: &str, new_text: &str) -> WordDiff {
    let old_words = word_tokens(old_text);
    let new_words = word_tokens(new_text);
    let table = lcs_table(&old_words, &new_words);
    let mut operations = Vec::new();
    let mut old_index = 0;
    let mut new_index = 0;

    while old_index < old_words.len() && new_index < new_words.len() {
        if old_words[old_index] == new_words[new_index] {
            operations.push(WordDiffOperation::Equal(old_words[old_index].to_owned()));
            old_index += 1;
            new_index += 1;
        } else if table[old_index + 1][new_index] >= table[old_index][new_index + 1] {
            operations.push(WordDiffOperation::Delete(old_words[old_index].to_owned()));
            old_index += 1;
        } else {
            operations.push(WordDiffOperation::Insert(new_words[new_index].to_owned()));
            new_index += 1;
        }
    }

    operations.extend(
        old_words[old_index..]
            .iter()
            .map(|word| WordDiffOperation::Delete((*word).to_owned())),
    );
    operations.extend(
        new_words[new_index..]
            .iter()
            .map(|word| WordDiffOperation::Insert((*word).to_owned())),
    );

    WordDiff { operations }
}

/// Tree-sitter parser wrapper enabled only for semantic builds.
#[cfg(feature = "semantic-tree-sitter")]
pub struct TreeSitterSemanticParser {
    parser: tree_sitter::Parser,
}

#[cfg(feature = "semantic-tree-sitter")]
impl TreeSitterSemanticParser {
    /// Creates a parser wrapper without selecting a language yet.
    pub fn new() -> Self {
        Self {
            parser: tree_sitter::Parser::new(),
        }
    }

    /// Returns mutable access for language adapters.
    pub fn parser_mut(&mut self) -> &mut tree_sitter::Parser {
        &mut self.parser
    }
}

#[cfg(feature = "semantic-tree-sitter")]
impl Default for TreeSitterSemanticParser {
    fn default() -> Self {
        Self::new()
    }
}

fn word_tokens(text: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut token_start = None;

    for (index, character) in text.char_indices() {
        if character.is_alphanumeric() || character == '_' {
            token_start.get_or_insert(index);
        } else {
            if let Some(start) = token_start.take() {
                tokens.push(&text[start..index]);
            }
            if !character.is_whitespace() {
                tokens.push(&text[index..index + character.len_utf8()]);
            }
        }
    }

    if let Some(start) = token_start {
        tokens.push(&text[start..]);
    }

    tokens
}

fn lcs_table(old_words: &[&str], new_words: &[&str]) -> Vec<Vec<usize>> {
    let mut table = vec![vec![0; new_words.len() + 1]; old_words.len() + 1];
    for old_index in (0..old_words.len()).rev() {
        for new_index in (0..new_words.len()).rev() {
            table[old_index][new_index] = if old_words[old_index] == new_words[new_index] {
                table[old_index + 1][new_index + 1] + 1
            } else {
                table[old_index + 1][new_index].max(table[old_index][new_index + 1])
            };
        }
    }
    table
}
