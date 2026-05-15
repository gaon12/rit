use std::fs;
use std::io::{self, Read, Write};

pub enum PathspecFileRead {
    Pathspecs(Vec<String>),
    Error { exit_code: u8 },
}

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
) -> io::Result<PathspecFileRead> {
    let data = if file_name.is_empty() {
        Vec::new()
    } else if file_name == "-" {
        let mut data = Vec::new();
        if let Err(error) = io::stdin().read_to_end(&mut data) {
            writeln!(
                stderr,
                "rit: {command} could not read pathspecs from stdin: {error}"
            )?;
            return Ok(PathspecFileRead::Error { exit_code: 129 });
        }
        data
    } else {
        match fs::read(file_name) {
            Ok(data) => data,
            Err(error) => {
                writeln!(
                    stderr,
                    "fatal: could not open '{}' for reading: {}",
                    file_name,
                    git_file_read_error(&error)
                )?;
                return Ok(PathspecFileRead::Error { exit_code: 128 });
            }
        }
    };

    if nul_terminated {
        if data.is_empty() {
            return Ok(PathspecFileRead::Pathspecs(Vec::new()));
        }
        let mut pathspecs = Vec::new();
        let parts = data.split(|byte| *byte == 0).collect::<Vec<_>>();
        for (index, item) in parts.iter().enumerate() {
            let is_trailing_empty =
                item.is_empty() && index + 1 == parts.len() && data.ends_with(&[0]);
            if item.is_empty() {
                if is_trailing_empty {
                    continue;
                }
                writeln!(
                    stderr,
                    "fatal: empty string is not a valid pathspec. please use . instead if you meant to match all paths"
                )?;
                return Ok(PathspecFileRead::Error { exit_code: 128 });
            }
            pathspecs.push(String::from_utf8_lossy(item).into_owned());
        }
        return Ok(PathspecFileRead::Pathspecs(pathspecs));
    }

    let text = String::from_utf8_lossy(&data);
    let mut pathspecs = Vec::new();
    for line in text.lines() {
        let line = line.split('\0').next().unwrap_or(line);
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() {
            writeln!(
                stderr,
                "fatal: empty string is not a valid pathspec. please use . instead if you meant to match all paths"
            )?;
            return Ok(PathspecFileRead::Error { exit_code: 128 });
        }
        match parse_pathspec_file_line(line) {
            Ok(pathspec) => pathspecs.push(pathspec),
            Err(()) => {
                writeln!(stderr, "fatal: line is badly quoted: {line}")?;
                return Ok(PathspecFileRead::Error { exit_code: 128 });
            }
        }
    }
    Ok(PathspecFileRead::Pathspecs(pathspecs))
}

fn parse_pathspec_file_line(line: &str) -> Result<String, ()> {
    if !line.starts_with('"') {
        return Ok(line.to_owned());
    }
    if !line.ends_with('"') || line.len() == 1 {
        return Err(());
    }

    let quoted = &line[1..line.len() - 1];
    let mut output = Vec::new();
    let mut chars = quoted.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            let mut bytes = [0; 4];
            output.extend_from_slice(ch.encode_utf8(&mut bytes).as_bytes());
            continue;
        }
        let Some(escaped) = chars.next() else {
            return Err(());
        };
        match escaped {
            '\\' => output.push(b'\\'),
            '"' => output.push(b'"'),
            'n' => output.push(b'\n'),
            'r' => output.push(b'\r'),
            't' => output.push(b'\t'),
            'a' => output.push(0x07),
            'b' => output.push(0x08),
            'f' => output.push(0x0c),
            'v' => output.push(0x0b),
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
                output.push(u8::try_from(value).map_err(|_| ())?);
            }
            _ => return Err(()),
        }
    }
    String::from_utf8(output).map_err(|_| ())
}

fn git_file_read_error(error: &io::Error) -> String {
    if error.kind() == io::ErrorKind::NotFound {
        "No such file or directory".to_owned()
    } else {
        error.to_string()
    }
}
