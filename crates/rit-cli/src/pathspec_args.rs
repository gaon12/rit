use std::fs;
use std::io::{self, Write};

pub fn handle_pathspec_file_option(
    args: &[String],
    index: &mut usize,
    after_separator: bool,
    pathspec_file: &mut Option<String>,
    pathspec_file_nul: &mut bool,
) -> io::Result<bool> {
    if after_separator {
        return Ok(false);
    }
    let arg = &args[*index];
    if arg == "--pathspec-file-nul" {
        *pathspec_file_nul = true;
        return Ok(true);
    }
    let file_name = if arg == "--pathspec-from-file" {
        *index += 1;
        let Some(value) = args.get(*index) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "option requires an argument: --pathspec-from-file",
            ));
        };
        value.clone()
    } else if let Some(value) = arg.strip_prefix("--pathspec-from-file=") {
        value.to_owned()
    } else {
        return Ok(false);
    };

    *pathspec_file = Some(file_name);
    Ok(true)
}

pub fn read_pathspecs_from_file(
    file_name: &str,
    nul_terminated: bool,
    command: &str,
    stderr: &mut dyn Write,
) -> io::Result<Option<Vec<String>>> {
    if file_name == "-" {
        writeln!(
            stderr,
            "rit: {command} does not support --pathspec-from-file=- yet"
        )?;
        return Ok(None);
    }

    let data = match fs::read(file_name) {
        Ok(data) => data,
        Err(error) => {
            writeln!(
                stderr,
                "rit: could not read pathspec file '{file_name}': {error}"
            )?;
            return Ok(None);
        }
    };

    if nul_terminated {
        let pathspecs = data
            .split(|byte| *byte == 0)
            .filter(|item| !item.is_empty())
            .map(|item| String::from_utf8_lossy(item).into_owned())
            .collect();
        return Ok(Some(pathspecs));
    }

    let text = match String::from_utf8(data) {
        Ok(text) => text,
        Err(error) => {
            writeln!(
                stderr,
                "rit: pathspec file '{file_name}' is not valid UTF-8: {error}"
            )?;
            return Ok(None);
        }
    };
    let pathspecs = text
        .lines()
        .map(|line| line.strip_suffix('\r').unwrap_or(line).to_owned())
        .filter(|line| !line.is_empty())
        .collect();
    Ok(Some(pathspecs))
}
