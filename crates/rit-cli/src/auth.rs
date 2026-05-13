use std::io::{self, Write};
use std::process::ExitCode;

pub fn auth_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let location = match args {
        [subcommand, location] if subcommand == "explain" && !location.starts_with('-') => location,
        [subcommand] if subcommand == "explain" => {
            writeln!(stderr, "rit: auth explain requires one URL or location")?;
            return Ok(ExitCode::from(129));
        }
        [subcommand, ..] => {
            writeln!(stderr, "rit: unsupported auth subcommand '{subcommand}'")?;
            return Ok(ExitCode::from(129));
        }
        [] => {
            writeln!(stderr, "rit: auth requires a subcommand")?;
            return Ok(ExitCode::from(129));
        }
    };

    let explanation = rit_core::explain_auth_location(location);
    writeln!(stdout, "auth: explain")?;
    writeln!(stdout, "location: {}", explanation.location)?;
    writeln!(stdout, "protocol: {}", explanation.protocol.label())?;
    writeln!(
        stdout,
        "credential-lookup: {}",
        explanation.uses_credentials()
    )?;
    if let Some(request) = explanation.credential_request {
        writeln!(stdout, "request-protocol: {}", request.protocol)?;
        writeln!(stdout, "request-host: {}", request.host)?;
        match request.path {
            Some(path) => writeln!(stdout, "request-path: {path}")?,
            None => writeln!(stdout, "request-path: <none>")?,
        }
        match request.username {
            Some(username) => writeln!(stdout, "request-username: {username}")?,
            None => writeln!(stdout, "request-username: <none>")?,
        }
    }
    if explanation.available_environment_tokens.is_empty() {
        writeln!(stdout, "environment-token: <none>")?;
    } else {
        for variable in explanation.available_environment_tokens {
            writeln!(stdout, "environment-token: {variable}")?;
        }
    }
    for note in explanation.notes {
        writeln!(stdout, "note: {note}")?;
    }
    Ok(ExitCode::SUCCESS)
}
