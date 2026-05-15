use std::io::{self, Write};
use std::process::ExitCode;

#[cfg(feature = "indexdb")]
pub fn indexdb_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let repository = match crate::discover_repository(stderr)? {
        Some(repository) => repository,
        None => return Ok(ExitCode::from(128)),
    };
    let manager = repository.indexdb();
    let result = match args {
        [] => manager.ensure().map(|result| {
            print_result("ensure", &result, stdout)?;
            Ok(ExitCode::SUCCESS)
        }),
        [subcommand] if subcommand == "status" => manager.status().map(|status| {
            print_status(&status, stdout)?;
            Ok(ExitCode::SUCCESS)
        }),
        [subcommand] if subcommand == "build" => manager.build().map(|result| {
            print_result("build", &result, stdout)?;
            Ok(ExitCode::SUCCESS)
        }),
        [subcommand] if subcommand == "update" => manager.update().map(|result| {
            print_result("update", &result, stdout)?;
            Ok(ExitCode::SUCCESS)
        }),
        [subcommand] if subcommand == "repair" => manager.repair().map(|result| {
            print_result("repair", &result, stdout)?;
            Ok(ExitCode::SUCCESS)
        }),
        [subcommand] if subcommand == "rebuild" => manager.rebuild().map(|result| {
            print_result("rebuild", &result, stdout)?;
            Ok(ExitCode::SUCCESS)
        }),
        [subcommand] if subcommand == "drop" => manager.drop().map(|()| {
            writeln!(stdout, "indexdb: drop")?;
            writeln!(
                stdout,
                "path: {}",
                manager.storage().database_path.display()
            )?;
            Ok(ExitCode::SUCCESS)
        }),
        [subcommand] if subcommand == "vacuum" => manager.vacuum().map(|status| {
            writeln!(stdout, "indexdb: vacuum")?;
            print_status(&status, stdout)?;
            Ok(ExitCode::SUCCESS)
        }),
        [subcommand, ..] => {
            writeln!(stderr, "rit: unsupported indexdb subcommand '{subcommand}'")?;
            return Ok(ExitCode::from(129));
        }
    };

    match result {
        Ok(code) => code,
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            Ok(ExitCode::from(1))
        }
    }
}

#[cfg(not(feature = "indexdb"))]
pub fn indexdb_command(
    _args: &[String],
    _stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    writeln!(
        stderr,
        "rit: this build does not include indexdb support; rebuild with the `indexdb` feature"
    )?;
    Ok(ExitCode::from(1))
}

#[cfg(feature = "indexdb")]
fn print_result(
    action: &str,
    result: &rit_core::IndexDbEnsureResult,
    stdout: &mut dyn Write,
) -> io::Result<()> {
    writeln!(stdout, "indexdb: {action}")?;
    writeln!(stdout, "created: {}", result.created)?;
    writeln!(stdout, "updated: {}", result.updated)?;
    writeln!(stdout, "commits-indexed: {}", result.commits_indexed)?;
    print_status(&result.status, stdout)
}

#[cfg(feature = "indexdb")]
fn print_status(status: &rit_core::IndexDbStatus, stdout: &mut dyn Write) -> io::Result<()> {
    writeln!(stdout, "path: {}", status.storage.database_path.display())?;
    writeln!(stdout, "lock-path: {}", status.storage.lock_path.display())?;
    writeln!(
        stdout,
        "worktree-cache-path: {}",
        status.storage.worktree_cache_path.display()
    )?;
    writeln!(
        stdout,
        "worktree-lock-path: {}",
        status.storage.worktree_lock_path.display()
    )?;
    writeln!(stdout, "exists: {}", status.exists)?;
    match status.schema_version {
        Some(version) => writeln!(stdout, "schema-version: {version}")?,
        None => writeln!(stdout, "schema-version: <none>")?,
    }
    writeln!(stdout, "healthy: {}", status.healthy)?;
    writeln!(stdout, "stale: {}", status.stale)?;
    if status.stale_reasons.is_empty() {
        writeln!(stdout, "stale-reason: <none>")?;
    } else {
        for reason in &status.stale_reasons {
            writeln!(stdout, "stale-reason: {reason}")?;
        }
    }
    Ok(())
}
