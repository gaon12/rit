use crate::{Result, RitError};
use toml::{Table, Value};

/// Repository policy configuration from `rit.toml`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyConfig {
    /// Optional maximum regular Git blob size in bytes.
    pub max_regular_blob_size: Option<u64>,
    /// Whether secret-pattern scanning is enabled.
    pub deny_secrets: bool,
    /// Branch names protected by policy.
    pub protect_branches: Vec<String>,
    /// Whether policy violations warn or block.
    pub enforcement: PolicyEnforcement,
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            max_regular_blob_size: None,
            deny_secrets: false,
            protect_branches: Vec::new(),
            enforcement: PolicyEnforcement::Warn,
        }
    }
}

/// Policy enforcement mode.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PolicyEnforcement {
    /// Report policy findings without blocking writes.
    #[default]
    Warn,
    /// Block writes when a policy violation is found.
    Block,
}

impl PolicyConfig {
    /// Parses `[policy]` from a TOML table.
    pub fn parse_from_table(table: &Table) -> Result<Self> {
        let Some(value) = table.get("policy") else {
            return Ok(Self::default());
        };
        let Some(policy) = value.as_table() else {
            return Err(RitError::invalid_input(
                "`policy` must be a TOML table in rit config",
            ));
        };

        Ok(Self {
            max_regular_blob_size: parse_optional_size(
                policy.get("max_regular_blob_size"),
                "policy.max_regular_blob_size",
            )?,
            deny_secrets: parse_optional_bool(policy.get("deny_secrets"), "policy.deny_secrets")?
                .unwrap_or(false),
            protect_branches: parse_optional_string_array(
                policy.get("protect_branches"),
                "policy.protect_branches",
            )?
            .unwrap_or_default(),
            enforcement: parse_enforcement(policy.get("enforcement"))?,
        })
    }

    /// Returns true when violations should block writes.
    pub fn blocks_writes(&self) -> bool {
        self.enforcement == PolicyEnforcement::Block
    }
}

/// Parses a human size limit such as `100 MiB`.
pub fn parse_size_limit(input: &str) -> Result<u64> {
    let trimmed = input.trim();
    let mut parts = trimmed.split_whitespace();
    let Some(number) = parts.next() else {
        return Err(RitError::invalid_input("size limit cannot be empty"));
    };
    let value = number
        .parse::<u64>()
        .map_err(|_| RitError::invalid_input(format!("invalid size limit number: {input}")))?;
    let unit = parts.next().unwrap_or("B").to_ascii_lowercase();
    if parts.next().is_some() {
        return Err(RitError::invalid_input(format!(
            "invalid size limit format: {input}"
        )));
    }

    let multiplier = match unit.as_str() {
        "b" | "byte" | "bytes" => 1,
        "kb" => 1_000,
        "mb" => 1_000_000,
        "gb" => 1_000_000_000,
        "kib" => 1024,
        "mib" => 1024 * 1024,
        "gib" => 1024 * 1024 * 1024,
        _ => {
            return Err(RitError::invalid_input(format!(
                "unsupported size limit unit: {unit}"
            )));
        }
    };
    value
        .checked_mul(multiplier)
        .ok_or_else(|| RitError::invalid_input(format!("size limit is too large: {input}")))
}

fn parse_optional_size(value: Option<&Value>, field_name: &str) -> Result<Option<u64>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if let Some(size) = value.as_integer() {
        return u64::try_from(size)
            .map(Some)
            .map_err(|_| RitError::invalid_input(format!("`{field_name}` must not be negative")));
    }
    value
        .as_str()
        .map(parse_size_limit)
        .transpose()?
        .map(Some)
        .ok_or_else(|| {
            RitError::invalid_input(format!("`{field_name}` must be a string or integer"))
        })
}

fn parse_optional_bool(value: Option<&Value>, field_name: &str) -> Result<Option<bool>> {
    let Some(value) = value else {
        return Ok(None);
    };
    value
        .as_bool()
        .map(Some)
        .ok_or_else(|| RitError::invalid_input(format!("`{field_name}` must be a boolean")))
}

fn parse_optional_string_array(
    value: Option<&Value>,
    field_name: &str,
) -> Result<Option<Vec<String>>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let Some(items) = value.as_array() else {
        return Err(RitError::invalid_input(format!(
            "`{field_name}` must be a string array"
        )));
    };
    items
        .iter()
        .map(|item| {
            item.as_str().map(str::to_owned).ok_or_else(|| {
                RitError::invalid_input(format!("`{field_name}` entries must be strings"))
            })
        })
        .collect::<Result<Vec<_>>>()
        .map(Some)
}

fn parse_enforcement(value: Option<&Value>) -> Result<PolicyEnforcement> {
    let Some(value) = value else {
        return Ok(PolicyEnforcement::Warn);
    };
    match value.as_str() {
        Some("warn") => Ok(PolicyEnforcement::Warn),
        Some("block") => Ok(PolicyEnforcement::Block),
        Some(other) => Err(RitError::invalid_input(format!(
            "`policy.enforcement` must be `warn` or `block`, got `{other}`"
        ))),
        None => Err(RitError::invalid_input(
            "`policy.enforcement` must be a string",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_policy_config_with_explicit_blocking() {
        let table = r#"
            [policy]
            max_regular_blob_size = "100 MiB"
            deny_secrets = true
            protect_branches = ["main", "release"]
            enforcement = "block"
        "#
        .parse::<Table>()
        .expect("TOML should parse");

        let policy = PolicyConfig::parse_from_table(&table).expect("policy should parse");

        assert_eq!(policy.max_regular_blob_size, Some(100 * 1024 * 1024));
        assert!(policy.deny_secrets);
        assert_eq!(policy.protect_branches, vec!["main", "release"]);
        assert!(policy.blocks_writes());
    }

    #[test]
    fn policy_defaults_warn_and_do_not_block() {
        let policy = PolicyConfig::parse_from_table(&Table::new()).expect("policy should parse");

        assert_eq!(policy, PolicyConfig::default());
        assert!(!policy.blocks_writes());
    }
}
