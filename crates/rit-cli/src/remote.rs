use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use crate::{capture_operation_snapshot, record_operation};

pub fn clone_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let mut quiet = false;
    let mut local = false;
    let mut no_checkout = false;
    let mut copy_tags = true;
    let mut origin_name = "origin".to_owned();
    let mut branch_name = None;
    let mut positional = Vec::new();
    let mut after_separator = false;
    let mut pending_option: Option<&'static str> = None;

    for arg in args {
        if let Some(option) = pending_option.take() {
            match option {
                "--origin" => origin_name = arg.to_owned(),
                "--branch" => branch_name = Some(arg.to_owned()),
                _ => unreachable!("unknown pending clone option"),
            }
            continue;
        }

        match arg.as_str() {
            "--" if !after_separator => after_separator = true,
            "-q" | "--quiet" if !after_separator => quiet = true,
            "-l" | "--local" if !after_separator => local = true,
            "-n" | "--no-checkout" if !after_separator => no_checkout = true,
            "--no-hardlinks" if !after_separator => {}
            "--tags" if !after_separator => copy_tags = true,
            "--no-tags" if !after_separator => copy_tags = false,
            "-o" | "--origin" if !after_separator => pending_option = Some("--origin"),
            "-b" | "--branch" if !after_separator => pending_option = Some("--branch"),
            option if option.starts_with("--origin=") && !after_separator => {
                origin_name = option.trim_start_matches("--origin=").to_owned();
            }
            option if option.starts_with("--branch=") && !after_separator => {
                branch_name = Some(option.trim_start_matches("--branch=").to_owned());
            }
            unsupported if unsupported.starts_with('-') && !after_separator => {
                writeln!(stderr, "rit: unsupported clone option '{unsupported}'")?;
                return Ok(ExitCode::from(129));
            }
            value => positional.push(value.to_owned()),
        }
    }
    if let Some(option) = pending_option {
        writeln!(stderr, "rit: clone option '{option}' requires a value")?;
        return Ok(ExitCode::from(129));
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
    let mut options = rit_core::LocalCloneOptions::new(&source, &directory)
        .with_origin_name(origin_name)
        .with_copy_tags(copy_tags);
    if let Some(branch_name) = branch_name {
        options = options.with_branch_name(branch_name);
    }

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
    let before = capture_operation_snapshot(&repository, stderr)?;

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
                    record_fetch_operation(&repository, &result.source, before, stderr)?;
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
                    record_fetch_operation(&repository, &result.source, before, stderr)?;
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
                    record_fetch_operation(&repository, &result.source, before, stderr)?;
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
    let before = capture_operation_snapshot(&repository, stderr)?;
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
            record_operation(
                &repository,
                "push",
                &format!("push {}", result.destination),
                before,
                Vec::new(),
                stderr,
            )?;
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

fn record_fetch_operation(
    repository: &rit_core::Repository,
    source: &str,
    before: Option<rit_core::OperationSnapshot>,
    stderr: &mut dyn Write,
) -> io::Result<()> {
    record_operation(
        repository,
        "fetch",
        &format!("fetch {source}"),
        before,
        Vec::new(),
        stderr,
    )
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
