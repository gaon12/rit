use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

pub fn clone_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let mut quiet = false;
    let mut local = false;
    let mut no_checkout = false;
    let mut positional = Vec::new();
    let mut after_separator = false;

    for arg in args {
        match arg.as_str() {
            "--" if !after_separator => after_separator = true,
            "-q" | "--quiet" if !after_separator => quiet = true,
            "-l" | "--local" if !after_separator => local = true,
            "-n" | "--no-checkout" if !after_separator => no_checkout = true,
            unsupported if unsupported.starts_with('-') && !after_separator => {
                writeln!(stderr, "rit: unsupported clone option '{unsupported}'")?;
                return Ok(ExitCode::from(129));
            }
            value => positional.push(value.to_owned()),
        }
    }

    if !local {
        writeln!(stderr, "rit: clone currently requires --local")?;
        return Ok(ExitCode::from(129));
    }
    if !no_checkout {
        writeln!(
            stderr,
            "rit: clone checkout is not implemented; pass --no-checkout"
        )?;
        return Ok(ExitCode::from(129));
    }
    if positional.is_empty() || positional.len() > 2 {
        writeln!(
            stderr,
            "rit: clone expects <source> and optional <directory>"
        )?;
        return Ok(ExitCode::from(129));
    }

    let source_location = rit_core::TransportLocation::parse(&positional[0]);
    let Some(source) = source_location.local_path() else {
        writeln!(
            stderr,
            "rit: clone --local requires a local repository path"
        )?;
        return Ok(ExitCode::from(129));
    };
    let directory = positional
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| default_clone_directory(&source));
    let options = rit_core::LocalCloneOptions::new(&source, &directory);

    match rit_core::Repository::clone_local_no_checkout(&options) {
        Ok(_) => {
            if !quiet {
                writeln!(
                    stdout,
                    "Cloned local repository into '{}'.",
                    directory.display()
                )?;
            }
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            Ok(ExitCode::from(1))
        }
    }
}

pub fn fetch_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let mut quiet = false;
    let mut positional = Vec::new();
    let mut after_separator = false;

    for arg in args {
        match arg.as_str() {
            "--" if !after_separator => after_separator = true,
            "-q" | "--quiet" if !after_separator => quiet = true,
            unsupported if unsupported.starts_with('-') && !after_separator => {
                writeln!(stderr, "rit: unsupported fetch option '{unsupported}'")?;
                return Ok(ExitCode::from(129));
            }
            value => positional.push(value.to_owned()),
        }
    }

    if positional.is_empty() || positional.len() > 2 {
        writeln!(
            stderr,
            "rit: fetch expects <local-repository> and optional <src>:<dst>"
        )?;
        return Ok(ExitCode::from(129));
    }

    let source_location = rit_core::TransportLocation::parse(&positional[0]);
    let repository = match discover_repository(stderr)? {
        Some(repository) => repository,
        None => return Ok(ExitCode::from(128)),
    };

    let refspec = match positional.get(1) {
        Some(refspec) => match rit_core::FetchRefSpec::parse(refspec) {
            Ok(refspec) => Some(refspec),
            Err(error) => {
                writeln!(stderr, "rit: {error}")?;
                return Ok(ExitCode::from(129));
            }
        },
        None => None,
    };

    match source_location.protocol() {
        rit_core::TransportProtocol::Local => {
            let source = source_location
                .local_path()
                .expect("local paths are available");
            let mut options = rit_core::LocalFetchOptions::new(source);
            if let Some(refspec) = refspec {
                options = options.with_refspec(refspec);
            }
            match repository.fetch_local(&options) {
                Ok(result) => {
                    if !quiet {
                        writeln!(stdout, "From {}", result.source)?;
                        writeln!(stdout, " * branch            HEAD       -> FETCH_HEAD")?;
                    }
                    Ok(ExitCode::SUCCESS)
                }
                Err(error) => {
                    writeln!(stderr, "rit: {error}")?;
                    Ok(ExitCode::from(1))
                }
            }
        }
        rit_core::TransportProtocol::Http | rit_core::TransportProtocol::Https => {
            let mut options = rit_core::RemoteFetchOptions::new(source_location);
            if let Some(refspec) = refspec {
                options = options.with_refspec(refspec);
            }
            match repository.fetch_remote_http(&options) {
                Ok(result) => {
                    if !quiet {
                        writeln!(stdout, "From {}", result.source)?;
                        writeln!(stdout, " * branch            HEAD       -> FETCH_HEAD")?;
                    }
                    Ok(ExitCode::SUCCESS)
                }
                Err(error) => {
                    writeln!(stderr, "rit: {error}")?;
                    Ok(ExitCode::from(1))
                }
            }
        }
        rit_core::TransportProtocol::Ssh => {
            let mut options = rit_core::RemoteFetchOptions::new(source_location);
            if let Some(refspec) = refspec {
                options = options.with_refspec(refspec);
            }
            match repository.fetch_remote_ssh(&options) {
                Ok(result) => {
                    if !quiet {
                        writeln!(stdout, "From {}", result.source)?;
                        writeln!(stdout, " * branch            HEAD       -> FETCH_HEAD")?;
                    }
                    Ok(ExitCode::SUCCESS)
                }
                Err(error) => {
                    writeln!(stderr, "rit: {error}")?;
                    Ok(ExitCode::from(1))
                }
            }
        }
    }
}

pub fn push_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let mut quiet = false;
    let mut positional = Vec::new();
    let mut after_separator = false;

    for arg in args {
        match arg.as_str() {
            "--" if !after_separator => after_separator = true,
            "-q" | "--quiet" if !after_separator => quiet = true,
            unsupported if unsupported.starts_with('-') && !after_separator => {
                writeln!(stderr, "rit: unsupported push option '{unsupported}'")?;
                return Ok(ExitCode::from(129));
            }
            value => positional.push(value.to_owned()),
        }
    }

    if positional.len() != 2 {
        writeln!(stderr, "rit: push expects <repository> and <src>:<dst>")?;
        return Ok(ExitCode::from(129));
    }

    let location = rit_core::TransportLocation::parse(&positional[0]);
    let refspec = match rit_core::FetchRefSpec::parse(&positional[1]) {
        Ok(refspec) => refspec,
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            return Ok(ExitCode::from(129));
        }
    };
    let repository = match discover_repository(stderr)? {
        Some(repository) => repository,
        None => return Ok(ExitCode::from(128)),
    };
    let options = rit_core::RemotePushOptions::new(location, refspec);
    let result = match options.location.protocol() {
        rit_core::TransportProtocol::Http | rit_core::TransportProtocol::Https => {
            repository.push_remote_http(&options)
        }
        rit_core::TransportProtocol::Ssh => repository.push_remote_ssh(&options),
        rit_core::TransportProtocol::Local => {
            writeln!(
                stderr,
                "rit: push currently supports only http://, https://, or SSH smart remotes"
            )?;
            return Ok(ExitCode::from(129));
        }
    };
    match result {
        Ok(result) => {
            if !quiet {
                writeln!(stdout, "To {}", options.location.original())?;
                writeln!(
                    stdout,
                    " * pushed            {} objects",
                    result.object_count
                )?;
            }
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            Ok(ExitCode::from(1))
        }
    }
}

fn default_clone_directory(source: &std::path::Path) -> PathBuf {
    source
        .file_name()
        .filter(|name| !name.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("repository"))
}

fn discover_repository(stderr: &mut dyn Write) -> io::Result<Option<rit_core::Repository>> {
    match rit_core::Repository::discover(".") {
        Ok(repository) => Ok(Some(repository)),
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            Ok(None)
        }
    }
}
