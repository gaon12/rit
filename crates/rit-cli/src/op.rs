use std::io::{self, Write};

pub fn write_operation_log_json(
    stdout: &mut dyn Write,
    log: &rit_core::OperationLog,
) -> io::Result<()> {
    writeln!(stdout, "{{")?;
    writeln!(stdout, "  \"records\": [")?;
    for (index, record) in log.records.iter().rev().enumerate() {
        if index > 0 {
            writeln!(stdout, ",")?;
        }
        write_operation_record_json(stdout, record)?;
    }
    writeln!(stdout)?;
    writeln!(stdout, "  ],")?;
    writeln!(stdout, "  \"warnings\": [")?;
    for (index, warning) in log.warnings.iter().enumerate() {
        if index > 0 {
            writeln!(stdout, ",")?;
        }
        write!(
            stdout,
            "    {{\"line_number\": {}, \"message\": \"{}\"}}",
            warning.line_number,
            json_escape(&warning.message)
        )?;
    }
    writeln!(stdout)?;
    writeln!(stdout, "  ]")?;
    writeln!(stdout, "}}")
}

fn write_operation_record_json(
    stdout: &mut dyn Write,
    record: &rit_core::OperationRecord,
) -> io::Result<()> {
    writeln!(stdout, "    {{")?;
    writeln!(stdout, "      \"id\": \"{}\",", json_escape(&record.id))?;
    writeln!(
        stdout,
        "      \"command\": \"{}\",",
        json_escape(&record.command)
    )?;
    writeln!(
        stdout,
        "      \"summary\": \"{}\",",
        json_escape(&record.summary)
    )?;
    write_operation_snapshot_json(stdout, "before", &record.before)?;
    write_operation_snapshot_json(stdout, "after", &record.after)?;
    write!(
        stdout,
        "      \"changed_paths\": {}",
        json_string_array(&record.changed_paths)
    )?;
    writeln!(stdout, ",")?;
    let object_ids = record
        .created_object_ids
        .iter()
        .map(|object_id| object_id.to_hex())
        .collect::<Vec<_>>();
    writeln!(
        stdout,
        "      \"created_object_ids\": {}",
        json_string_array(&object_ids)
    )?;
    write!(stdout, "    }}")
}

fn write_operation_snapshot_json(
    stdout: &mut dyn Write,
    name: &str,
    snapshot: &rit_core::OperationSnapshot,
) -> io::Result<()> {
    writeln!(stdout, "      \"{name}\": {{")?;
    writeln!(
        stdout,
        "        \"head\": {},",
        json_optional_string(snapshot.head.map(|head| head.to_hex()).as_deref())
    )?;
    writeln!(
        stdout,
        "        \"branch\": {},",
        json_optional_string(snapshot.branch.as_deref())
    )?;
    writeln!(
        stdout,
        "        \"index_checksum\": {}",
        json_optional_string(snapshot.index_checksum.as_deref())
    )?;
    writeln!(stdout, "      }},")
}

fn json_string_array(values: &[String]) -> String {
    let quoted = values
        .iter()
        .map(|value| format!("\"{}\"", json_escape(value)))
        .collect::<Vec<_>>();
    format!("[{}]", quoted.join(", "))
}

fn json_optional_string(value: Option<&str>) -> String {
    value
        .map(|value| format!("\"{}\"", json_escape(value)))
        .unwrap_or_else(|| "null".to_owned())
}

fn json_escape(input: &str) -> String {
    let mut output = String::new();
    for character in input.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            '\u{08}' => output.push_str("\\b"),
            '\u{0c}' => output.push_str("\\f"),
            character if character.is_control() => {
                use std::fmt::Write as _;
                write!(output, "\\u{:04x}", character as u32)
                    .expect("writing to a String should not fail");
            }
            character => output.push(character),
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::json_escape;

    #[test]
    fn json_escape_handles_quotes_slashes_and_controls() {
        assert_eq!(
            json_escape("quote\" slash\\ newline\n tab\t"),
            "quote\\\" slash\\\\ newline\\n tab\\t"
        );
        assert_eq!(json_escape("\u{0001}"), "\\u0001");
    }
}
