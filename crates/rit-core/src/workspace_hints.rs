use crate::{Result, RitError, WorkspaceRecommendationHint};
use std::fs;
use std::path::Path;

pub(crate) fn relative_path_string(root: &Path, path: &Path) -> Result<String> {
    path.strip_prefix(root)
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .map_err(|_| RitError::invalid_input("path is outside the worktree"))
}

pub(crate) fn has_package_manifest(path: &Path) -> bool {
    package_manifest_name(path).is_some()
}

pub(crate) fn nearest_package_manifest(worktree: &Path, path: &str) -> Result<Option<String>> {
    let mut current = worktree.join(path.replace('/', std::path::MAIN_SEPARATOR_STR));
    if current.is_file() {
        current.pop();
    }
    loop {
        if let Some(name) = package_manifest_name(&current) {
            return Ok(Some(
                format!("{}/{}", relative_path_string(worktree, &current)?, name)
                    .trim_start_matches('/')
                    .to_owned(),
            ));
        }
        if !current.pop() || current == worktree {
            break;
        }
    }
    Ok(None)
}

fn package_manifest_name(path: &Path) -> Option<&'static str> {
    [
        "Cargo.toml",
        "package.json",
        "pyproject.toml",
        "go.mod",
        "pnpm-workspace.yaml",
        "turbo.json",
        "nx.json",
    ]
    .into_iter()
    .find(|name| path.join(name).is_file())
}

pub(crate) fn codeowners_hints(
    worktree: &Path,
    changed_paths: &[String],
) -> Result<Vec<WorkspaceRecommendationHint>> {
    let mut hints = Vec::new();
    for codeowners_path in [
        worktree.join(".github").join("CODEOWNERS"),
        worktree.join("CODEOWNERS"),
        worktree.join("docs").join("CODEOWNERS"),
    ] {
        if !codeowners_path.is_file() {
            continue;
        }
        let contents = fs::read_to_string(&codeowners_path)
            .map_err(|source| RitError::io(&codeowners_path, source))?;
        let codeowners_relative = relative_path_string(worktree, &codeowners_path)?;
        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut parts = line.split_whitespace();
            let Some(pattern) = parts.next() else {
                continue;
            };
            let owners = parts.collect::<Vec<_>>().join(" ");
            for path in changed_paths {
                if codeowners_pattern_matches(pattern, path) {
                    hints.push(WorkspaceRecommendationHint {
                        kind: "codeowners".to_owned(),
                        path: path.clone(),
                        detail: format!("{codeowners_relative}: {owners}"),
                    });
                }
            }
        }
    }
    Ok(hints)
}

fn codeowners_pattern_matches(pattern: &str, path: &str) -> bool {
    let pattern = pattern.trim_start_matches('/');
    if let Some(prefix) = pattern.strip_suffix('/') {
        return slash_path_matches_prefix(path, prefix);
    }
    if pattern.contains('*') {
        let target = if pattern.contains('/') {
            path
        } else {
            path.rsplit('/').next().unwrap_or(path)
        };
        return simple_star_match(pattern, target);
    }
    slash_path_matches_prefix(path, pattern)
}

fn slash_path_matches_prefix(path: &str, prefix: &str) -> bool {
    let path = path.trim_matches('/');
    let prefix = prefix.trim_matches('/');
    path == prefix || path.starts_with(&format!("{prefix}/"))
}

fn simple_star_match(pattern: &str, path: &str) -> bool {
    let parts = pattern
        .split('*')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let mut rest = path;
    for (index, part) in parts.iter().enumerate() {
        let Some(position) = rest.find(part) else {
            return false;
        };
        if index == 0 && !pattern.starts_with('*') && position != 0 {
            return false;
        }
        rest = &rest[position + part.len()..];
    }
    pattern.ends_with('*') || parts.last().is_none_or(|last| path.ends_with(last))
}

pub(crate) fn import_hints_for_path(
    worktree: &Path,
    path: &str,
) -> Result<Vec<WorkspaceRecommendationHint>> {
    if !is_source_path(path) {
        return Ok(Vec::new());
    }
    let full_path = worktree.join(path.replace('/', std::path::MAIN_SEPARATOR_STR));
    if !full_path.is_file() {
        return Ok(Vec::new());
    }
    let contents =
        fs::read_to_string(&full_path).map_err(|source| RitError::io(&full_path, source))?;
    let mut hints = Vec::new();
    for line in contents.lines().take(200) {
        let line = line.trim();
        if line.starts_with("use ")
            || line.starts_with("mod ")
            || line.starts_with("import ")
            || line.starts_with("from ")
            || line.contains(" require(")
        {
            hints.push(WorkspaceRecommendationHint {
                kind: "import-graph".to_owned(),
                path: path.to_owned(),
                detail: "source imports may connect this path to nearby packages".to_owned(),
            });
            break;
        }
    }
    Ok(hints)
}

fn is_source_path(path: &str) -> bool {
    [".rs", ".ts", ".tsx", ".js", ".jsx", ".py", ".go"]
        .into_iter()
        .any(|extension| path.ends_with(extension))
}

#[cfg(test)]
mod tests {
    use super::codeowners_pattern_matches;

    #[test]
    fn codeowners_patterns_match_prefixes_and_stars() {
        assert!(codeowners_pattern_matches(
            "/apps/mobile/",
            "apps/mobile/src/lib.rs"
        ));
        assert!(codeowners_pattern_matches("*.rs", "apps/mobile/src/lib.rs"));
        assert!(!codeowners_pattern_matches(
            "/services/api/",
            "apps/mobile/src/lib.rs"
        ));
    }
}
