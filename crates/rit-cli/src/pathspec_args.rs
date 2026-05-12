use std::fs;
use std::io::{self, Read, Write};

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
    let data = if file_name == "-" {
        let mut data = Vec::new();
        if let Err(error) = io::stdin().read_to_end(&mut data) {
            writeln!(
                stderr,
                "rit: {command} could not read pathspecs from stdin: {error}"
            )?;
            return Ok(None);
        }
        data
    } else {
        match fs::read(file_name) {
            Ok(data) => data,
            Err(error) => {
                writeln!(
                    stderr,
                    "rit: could not read pathspec file '{file_name}': {error}"
                )?;
                return Ok(None);
            }
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
    let mut pathspecs = Vec::new();
    for line in text.lines() {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() {
            continue;
        }
        match parse_pathspec_file_line(line) {
            Ok(pathspec) => pathspecs.push(pathspec),
            Err(error) => {
                writeln!(stderr, "rit: invalid pathspec file entry '{line}': {error}")?;
                return Ok(None);
            }
        }
    }
    Ok(Some(pathspecs))
}

fn parse_pathspec_file_line(line: &str) -> Result<String, String> {
    if !line.starts_with('"') {
        return Ok(line.to_owned());
    }
    if !line.ends_with('"') || line.len() == 1 {
        return Err("missing closing quote".to_owned());
    }

    let quoted = &line[1..line.len() - 1];
    let mut output = String::new();
    let mut chars = quoted.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            output.push(ch);
            continue;
        }
        let Some(escaped) = chars.next() else {
            return Err("trailing backslash escape".to_owned());
        };
        match escaped {
            '\\' => output.push('\\'),
            '"' => output.push('"'),
            'n' => output.push('\n'),
            'r' => output.push('\r'),
            't' => output.push('\t'),
            'b' => output.push('\u{0008}'),
            'f' => output.push('\u{000c}'),
            'v' => output.push('\u{000b}'),
            '0'..='7' => {
                let mut value = escaped.to_digit(8).unwrap_or(0);
                for _ in 0..2 {
                    let Some(next) = chars.peek().copied() else {
                        break;
                    };
                    if !('0'..='7').contains(&next) {
                        break;
                    }
                    chars.next();
                    value = value * 8 + next.to_digit(8).unwrap_or(0);
                }
                let Some(decoded) = char::from_u32(value) else {
                    return Err(format!("invalid octal escape: {value:o}"));
                };
                output.push(decoded);
            }
            other => return Err(format!("unsupported escape: \\{other}")),
        }
    }
    Ok(output)
}
