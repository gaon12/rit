use crate::{ObjectId, Result, RitError};

/// Parsed commit object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Commit {
    /// Tree object referenced by the commit.
    pub tree: ObjectId,
    /// Parent commits in stored order.
    pub parents: Vec<ObjectId>,
    /// Author identity and timestamp.
    pub author: Signature,
    /// Committer identity and timestamp.
    pub committer: Signature,
    /// Raw commit message without the header separator.
    pub message: String,
}

/// Git commit identity line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Signature {
    /// Display name.
    pub name: String,
    /// Email address without angle brackets.
    pub email: String,
    /// Seconds since Unix epoch.
    pub timestamp: i64,
    /// Time zone offset such as `+0900`.
    pub offset: String,
}

/// Parses a commit object payload.
pub fn parse_commit(data: &[u8]) -> Result<Commit> {
    let text = std::str::from_utf8(data)
        .map_err(|_| RitError::invalid_input("commit object is not UTF-8"))?;
    let (headers, message) = text.split_once("\n\n").unwrap_or((text, ""));
    let mut tree = None;
    let mut parents = Vec::new();
    let mut author = None;
    let mut committer = None;

    for line in headers.lines() {
        if let Some(value) = line.strip_prefix("tree ") {
            tree = Some(ObjectId::from_hex(value)?);
        } else if let Some(value) = line.strip_prefix("parent ") {
            parents.push(ObjectId::from_hex(value)?);
        } else if let Some(value) = line.strip_prefix("author ") {
            author = Some(parse_signature(value)?);
        } else if let Some(value) = line.strip_prefix("committer ") {
            committer = Some(parse_signature(value)?);
        }
    }

    Ok(Commit {
        tree: tree.ok_or_else(|| RitError::invalid_input("commit object is missing tree line"))?,
        parents,
        author: author
            .ok_or_else(|| RitError::invalid_input("commit object is missing author line"))?,
        committer: committer
            .ok_or_else(|| RitError::invalid_input("commit object is missing committer line"))?,
        message: message.to_owned(),
    })
}

fn parse_signature(value: &str) -> Result<Signature> {
    let Some(email_start) = value.rfind(" <") else {
        return Err(RitError::invalid_input("commit signature is missing email"));
    };
    let Some(email_end) = value[email_start + 2..].find("> ") else {
        return Err(RitError::invalid_input("commit signature is malformed"));
    };
    let email_end = email_start + 2 + email_end;
    let remainder = &value[email_end + 2..];
    let mut parts = remainder.split_whitespace();
    let timestamp = parts
        .next()
        .ok_or_else(|| RitError::invalid_input("commit signature is missing timestamp"))?
        .parse::<i64>()
        .map_err(|_| RitError::invalid_input("commit signature timestamp is invalid"))?;
    let offset = parts
        .next()
        .ok_or_else(|| RitError::invalid_input("commit signature is missing timezone"))?;

    Ok(Signature {
        name: value[..email_start].to_owned(),
        email: value[email_start + 2..email_end].to_owned(),
        timestamp,
        offset: offset.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::parse_commit;

    #[test]
    fn parses_basic_commit() {
        let commit = parse_commit(
            b"tree ce013625030ba8dba906f756967f9e9ca394464a\nparent b6fc4c620b67d95f953a5c1c1230aaab5db5a1b0\nauthor A U Thor <a@example.test> 1700000000 +0900\ncommitter C O Mitter <c@example.test> 1700000001 +0900\n\nSubject\n\nBody\n",
        )
        .expect("commit should parse");

        assert_eq!(commit.parents.len(), 1);
        assert_eq!(commit.author.name, "A U Thor");
        assert_eq!(commit.author.email, "a@example.test");
        assert_eq!(commit.message, "Subject\n\nBody\n");
    }
}
