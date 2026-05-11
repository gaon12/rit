use crate::{GitConfig, Result, RitError};
use std::fs;
use std::path::{Path, PathBuf};

/// Read-only partial-clone policy discovered from Git config and pack markers.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PartialClonePolicy {
    /// Promisor remotes configured in `.git/config`.
    pub promisor_remotes: Vec<PromisorRemote>,
    /// `.promisor` marker files next to packs.
    pub promisor_pack_markers: Vec<PathBuf>,
}

/// One remote that may provide promised-but-missing objects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromisorRemote {
    /// Remote name, such as `origin`.
    pub name: String,
    /// Git partial clone filter, such as `blob:none`.
    pub partial_clone_filter: Option<String>,
}

impl PartialClonePolicy {
    /// Reads partial-clone policy from config and object storage.
    pub fn read(config: &GitConfig, objects_dir: &Path) -> Result<Self> {
        Ok(Self {
            promisor_remotes: read_promisor_remotes(config)?,
            promisor_pack_markers: read_promisor_pack_markers(objects_dir)?,
        })
    }

    /// Returns true when this repository has any partial-clone/promisor signal.
    pub fn is_enabled(&self) -> bool {
        !self.promisor_remotes.is_empty() || !self.promisor_pack_markers.is_empty()
    }
}

fn read_promisor_remotes(config: &GitConfig) -> Result<Vec<PromisorRemote>> {
    let mut remotes = Vec::new();
    for remote_name in config.subsections_in_section("remote") {
        if !config.get_bool_in_subsection("remote", remote_name, "promisor", false)? {
            continue;
        }
        remotes.push(PromisorRemote {
            name: remote_name.to_owned(),
            partial_clone_filter: config
                .get_in_subsection("remote", Some(remote_name), "partialclonefilter")
                .map(str::to_owned),
        });
    }
    Ok(remotes)
}

fn read_promisor_pack_markers(objects_dir: &Path) -> Result<Vec<PathBuf>> {
    let pack_dir = objects_dir.join("pack");
    if !pack_dir.exists() {
        return Ok(Vec::new());
    }

    let mut markers = Vec::new();
    for entry in fs::read_dir(&pack_dir).map_err(|source| RitError::io(&pack_dir, source))? {
        let entry = entry.map_err(|source| RitError::io(&pack_dir, source))?;
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) == Some("promisor") {
            markers.push(path);
        }
    }
    markers.sort();
    Ok(markers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn reads_promisor_remotes_and_pack_markers() {
        let config = GitConfig::parse(
            r#"
            [remote "origin"]
                promisor = true
                partialCloneFilter = blob:none
            [remote "backup"]
                promisor = false
            "#,
        )
        .expect("config should parse");
        let objects_dir = temp_dir("promisor").join("objects");
        let pack_dir = objects_dir.join("pack");
        fs::create_dir_all(&pack_dir).expect("pack dir should be created");
        fs::write(pack_dir.join("pack-a.promisor"), "").expect("marker should be written");
        fs::write(pack_dir.join("pack-a.pack"), "").expect("pack should be written");

        let policy = PartialClonePolicy::read(&config, &objects_dir).expect("policy should read");

        assert!(policy.is_enabled());
        assert_eq!(
            policy.promisor_remotes,
            vec![PromisorRemote {
                name: "origin".to_owned(),
                partial_clone_filter: Some("blob:none".to_owned()),
            }]
        );
        assert_eq!(policy.promisor_pack_markers.len(), 1);

        let _ = fs::remove_dir_all(
            objects_dir
                .parent()
                .expect("objects dir should have temp parent"),
        );
    }

    #[test]
    fn empty_policy_is_disabled() {
        let config = GitConfig::default();
        let policy = PartialClonePolicy::read(&config, Path::new("missing-objects"))
            .expect("missing objects should be accepted");

        assert!(!policy.is_enabled());
    }

    fn temp_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be available")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("rit-partial-{name}-{suffix}"));
        let _ = fs::remove_dir_all(&path);
        path
    }
}
