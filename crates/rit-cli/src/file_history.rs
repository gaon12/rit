use std::io::{self, Write};
use std::process::ExitCode;

#[cfg(feature = "indexdb")]
pub fn file_history_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let path = match args {
        [path] if !path.starts_with('-') => path,
        [] => {
            writeln!(
                stderr,
                "rit: file-history requires one repository-relative path"
            )?;
            return Ok(ExitCode::from(129));
        }
        [flag] if flag.starts_with('-') => {
            writeln!(stderr, "rit: unsupported file-history option '{flag}'")?;
            return Ok(ExitCode::from(129));
        }
        [_, ..] => {
            writeln!(stderr, "rit: file-history accepts exactly one path")?;
            return Ok(ExitCode::from(129));
        }
    };
    let repository = match crate::discover_repository(stderr)? {
        Some(repository) => repository,
        None => return Ok(ExitCode::from(128)),
    };
    let manager = repository.indexdb();
    if let Err(error) = manager.ensure() {
        writeln!(stderr, "rit: {error}")?;
        return Ok(ExitCode::from(1));
    }
    let history = match manager.file_history(path) {
        Ok(history) => history,
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            return Ok(ExitCode::from(1));
        }
    };

    writeln!(stdout, "file-history: {path}")?;
    for change in history {
        let mode = change
            .mode
            .map(|mode| format!("{mode:06o}"))
            .unwrap_or_else(|| "<none>".to_owned());
        let object_id = change
            .object_id
            .map(|object_id| object_id.to_string())
            .unwrap_or_else(|| "<none>".to_owned());
        writeln!(
            stdout,
            "{} {} {} {} {}",
            change.commit_id, change.change_kind, mode, object_id, change.path
        )?;
    }
    Ok(ExitCode::SUCCESS)
}

#[cfg(not(feature = "indexdb"))]
pub fn file_history_command(
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
