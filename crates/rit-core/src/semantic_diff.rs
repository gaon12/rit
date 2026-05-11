/// Word-level diff result for higher-level semantic summaries.
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WordDiffOperation {
    /// Unchanged token.
    Equal(String),
    /// Token present only in the old text.
    Delete(String),
    /// Token present only in the new text.
    Insert(String),
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
