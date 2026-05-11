use crate::{Result, RitError};
use std::fs;
use std::path::{Path, PathBuf};
use toml::{Table, Value};

/// Parsed optional `rit.toml` repository configuration.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RitConfig {
    /// Path the config was read from, when one exists.
    pub path: Option<PathBuf>,
    /// Named workspace profiles.
    pub workspace_profiles: Vec<WorkspaceProfile>,
}

/// A named user-facing workspace profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceProfile {
    /// Profile name from `[workspace.<name>]`.
    pub name: String,
    /// Repository-relative paths included by this workspace.
    pub include: Vec<String>,
}

impl RitConfig {
    /// Reads `rit.toml` or `.rit.toml` from a working tree root.
    pub fn read_from_worktree(worktree: &Path) -> Result<Self> {
        let Some(path) = find_rit_config(worktree) else {
            return Ok(Self::default());
        };
        Self::read(&path)
    }

    /// Reads one explicit rit config file.
    pub fn read(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path).map_err(|source| RitError::io(path, source))?;
        let mut config = Self::parse(&contents)?;
        config.path = Some(path.to_path_buf());
        Ok(config)
    }

    /// Parses rit TOML configuration.
    pub fn parse(contents: &str) -> Result<Self> {
        let table = contents.parse::<Table>().map_err(|error| {
            RitError::invalid_input(format!("failed to parse rit config TOML: {error}"))
        })?;
        let workspace_profiles = parse_workspace_profiles(&table)?;
        Ok(Self {
            path: None,
            workspace_profiles,
        })
    }

    /// Returns one named workspace profile.
    pub fn workspace_profile(&self, name: &str) -> Option<&WorkspaceProfile> {
        self.workspace_profiles
            .iter()
            .find(|profile| profile.name == name)
    }
}

fn find_rit_config(worktree: &Path) -> Option<PathBuf> {
    ["rit.toml", ".rit.toml"]
        .into_iter()
        .map(|name| worktree.join(name))
        .find(|path| path.is_file())
}

fn parse_workspace_profiles(table: &Table) -> Result<Vec<WorkspaceProfile>> {
    let Some(workspace) = table.get("workspace") else {
        return Ok(Vec::new());
    };
    let Some(workspace_table) = workspace.as_table() else {
        return Err(RitError::invalid_input(
            "`workspace` must be a TOML table in rit config",
        ));
    };

    let mut profiles = Vec::new();
    for (name, profile_value) in workspace_table {
        let Some(profile_table) = profile_value.as_table() else {
            return Err(RitError::invalid_input(format!(
                "`workspace.{name}` must be a TOML table"
            )));
        };
        let include = match profile_table.get("include") {
            Some(include_value) => {
                parse_string_array(include_value, &format!("workspace.{name}.include"))?
            }
            None => Vec::new(),
        };
        profiles.push(WorkspaceProfile {
            name: name.to_owned(),
            include,
        });
    }
    Ok(profiles)
}

fn parse_string_array(value: &Value, field_name: &str) -> Result<Vec<String>> {
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
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parses_workspace_profiles() {
        let config = RitConfig::parse(
            r#"
            [workspace.mobile]
            include = ["apps/mobile", "packages/ui"]

            [workspace.backend]
            include = ["services/api"]
            "#,
        )
        .expect("config should parse");

        assert_eq!(config.workspace_profiles.len(), 2);
        assert_eq!(
            config
                .workspace_profile("mobile")
                .expect("mobile should exist")
                .include,
            vec!["apps/mobile", "packages/ui"]
        );
        assert_eq!(
            config
                .workspace_profile("backend")
                .expect("backend should exist")
                .include,
            vec!["services/api"]
        );
    }

    #[test]
    fn rejects_invalid_include_arrays() {
        let error = RitConfig::parse(
            r#"
            [workspace.mobile]
            include = "apps/mobile"
            "#,
        )
        .expect_err("invalid include should fail");

        assert!(error.to_string().contains("workspace.mobile.include"));
    }

    #[test]
    fn reads_rit_toml_before_dot_rit_toml() {
        let worktree = temp_dir("config-order");
        fs::create_dir_all(&worktree).expect("worktree should be created");
        fs::write(
            worktree.join("rit.toml"),
            "[workspace.first]\ninclude = [\"first\"]\n",
        )
        .expect("rit config should be written");
        fs::write(
            worktree.join(".rit.toml"),
            "[workspace.second]\ninclude = [\"second\"]\n",
        )
        .expect("dot rit config should be written");

        let config = RitConfig::read_from_worktree(&worktree).expect("config should read");

        assert!(config.path.as_deref().is_some_and(|path| {
            path.file_name().and_then(|name| name.to_str()) == Some("rit.toml")
        }));
        assert!(config.workspace_profile("first").is_some());
        assert!(config.workspace_profile("second").is_none());

        let _ = fs::remove_dir_all(worktree);
    }

    fn temp_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be available")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("rit-workspace-{name}-{suffix}"));
        let _ = fs::remove_dir_all(&path);
        path
    }
}
