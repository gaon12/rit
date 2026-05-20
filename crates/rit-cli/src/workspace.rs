use std::io::{self, Write};
use std::process::ExitCode;

use crate::discover_repository;

pub(super) fn workspace_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let recommendation_mode = match args {
        [subcommand] if subcommand == "suggest" || subcommand == "from-change" => {
            Some(subcommand.as_str())
        }
        [subcommand, package_path]
            if subcommand == "from-package" && !package_path.starts_with('-') =>
        {
            Some(subcommand.as_str())
        }
        [subcommand] if subcommand == "from-package" => {
            writeln!(stderr, "rit: workspace from-package requires a path")?;
            return Ok(ExitCode::from(129));
        }
        _ => None,
    };
    if let Some(mode) = recommendation_mode {
        let repository = match discover_repository(stderr)? {
            Some(repository) => repository,
            None => return Ok(ExitCode::from(128)),
        };
        let report = match mode {
            "suggest" => repository.workspace_suggestions(),
            "from-change" => repository.workspace_from_change(),
            "from-package" => repository.workspace_from_package(&args[1]),
            _ => unreachable!("recommendation mode was matched above"),
        };
        return match report {
            Ok(report) => {
                print_workspace_recommendation_report(&report, stdout)?;
                Ok(ExitCode::SUCCESS)
            }
            Err(error) => {
                writeln!(stderr, "rit: {error}")?;
                Ok(ExitCode::from(1))
            }
        };
    }

    let (mode, profile_name) = match args {
        [subcommand, profile]
            if (subcommand == "prefetch" || subcommand == "explain")
                && !profile.starts_with('-') =>
        {
            (subcommand.as_str(), profile)
        }
        [subcommand] if subcommand == "prefetch" => {
            writeln!(stderr, "rit: workspace prefetch requires a profile name")?;
            return Ok(ExitCode::from(129));
        }
        [subcommand] if subcommand == "explain" => {
            writeln!(stderr, "rit: workspace explain requires a profile name")?;
            return Ok(ExitCode::from(129));
        }
        [subcommand, ..] => {
            writeln!(
                stderr,
                "rit: unsupported workspace subcommand '{subcommand}'"
            )?;
            return Ok(ExitCode::from(129));
        }
        [] => {
            writeln!(stderr, "rit: workspace requires a subcommand")?;
            return Ok(ExitCode::from(129));
        }
    };

    let repository = match discover_repository(stderr)? {
        Some(repository) => repository,
        None => return Ok(ExitCode::from(128)),
    };
    let config = match repository.rit_config() {
        Ok(config) => config,
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            return Ok(ExitCode::from(1));
        }
    };
    let Some(profile) = config.workspace_profile(profile_name) else {
        writeln!(stderr, "rit: workspace profile not found: {profile_name}")?;
        return Ok(ExitCode::from(1));
    };
    let partial_clone = match repository.partial_clone_policy() {
        Ok(policy) => policy,
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            return Ok(ExitCode::from(1));
        }
    };
    let plan = profile.prefetch_plan(&partial_clone);

    writeln!(stdout, "workspace: {}", plan.workspace)?;
    if mode == "explain" {
        let explanation = profile.explain_decisions(&partial_clone);
        writeln!(stdout, "explain: decisions")?;
        print_workspace_prefetch_plan(&explanation.plan, stdout)?;
        for decision in explanation.decisions {
            writeln!(stdout, "decision: {}", decision.name)?;
            writeln!(stdout, "selected: {}", decision.selected)?;
            writeln!(stdout, "reason: {}", decision.reason)?;
        }
        return Ok(ExitCode::SUCCESS);
    }

    print_workspace_prefetch_plan(&plan, stdout)?;
    Ok(ExitCode::SUCCESS)
}

fn print_workspace_recommendation_report(
    report: &rit_core::WorkspaceRecommendationReport,
    stdout: &mut dyn Write,
) -> io::Result<()> {
    writeln!(stdout, "workspace: recommendation")?;
    match &report.mode {
        rit_core::WorkspaceRecommendationMode::CurrentChanges => {
            writeln!(stdout, "source: current-changes")?
        }
        rit_core::WorkspaceRecommendationMode::PackagePath(path) => {
            writeln!(stdout, "source: package-path")?;
            writeln!(stdout, "package-path: {path}")?;
        }
    }
    if let Some(package_root) = &report.package_root {
        writeln!(stdout, "package-root: {package_root}")?;
    }
    for path in &report.changed_paths {
        writeln!(stdout, "changed: {path}")?;
    }
    if report.recommendations.is_empty() {
        writeln!(stdout, "recommendation: (none)")?;
    }
    for recommendation in &report.recommendations {
        writeln!(stdout, "recommendation: {}", recommendation.workspace)?;
        writeln!(stdout, "score: {}", recommendation.score)?;
        for include in &recommendation.include {
            writeln!(stdout, "include: {include}")?;
        }
        for matched_path in &recommendation.matched_paths {
            writeln!(stdout, "match: {matched_path}")?;
        }
        for reason in &recommendation.reasons {
            writeln!(stdout, "reason: {reason}")?;
        }
    }
    for hint in &report.hints {
        writeln!(stdout, "hint: {} {} {}", hint.kind, hint.path, hint.detail)?;
    }
    Ok(())
}

fn print_workspace_prefetch_plan(
    plan: &rit_core::WorkspacePrefetchPlan,
    stdout: &mut dyn Write,
) -> io::Result<()> {
    writeln!(stdout, "prefetch: planned")?;
    writeln!(stdout, "partial-clone: {}", plan.partial_clone)?;
    writeln!(stdout, "lazy-files: {}", plan.lazy_files)?;
    if let Some(remote) = &plan.promisor_remote {
        writeln!(stdout, "promisor-remote: {remote}")?;
    }
    if let Some(filter) = &plan.partial_clone_filter {
        writeln!(stdout, "partial-clone-filter: {filter}")?;
    }
    for path in &plan.include {
        writeln!(stdout, "include: {path}")?;
    }
    Ok(())
}
