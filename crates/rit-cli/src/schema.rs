use std::io::{self, Write};
use std::process::ExitCode;

pub fn schema_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    match args {
        [] => {
            writeln!(stderr, "rit: schema requires a name")?;
            writeln!(stderr, "Available schemas: {}", schema_names().join(", "))?;
            Ok(ExitCode::from(129))
        }
        [name] => match rit_core::json_schema(name) {
            Some(schema) => {
                writeln!(stdout, "{}", schema.json)?;
                Ok(ExitCode::SUCCESS)
            }
            None => {
                writeln!(stderr, "rit: unknown schema '{name}'")?;
                writeln!(stderr, "Available schemas: {}", schema_names().join(", "))?;
                Ok(ExitCode::from(129))
            }
        },
        _ => {
            writeln!(stderr, "rit: schema accepts exactly one name")?;
            Ok(ExitCode::from(129))
        }
    }
}

fn schema_names() -> Vec<&'static str> {
    rit_core::json_schemas()
        .iter()
        .map(|schema| schema.name)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::schema_command;

    #[test]
    fn schema_command_prints_status_schema() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = schema_command(&["status".to_owned()], &mut stdout, &mut stderr)
            .expect("schema command should run");
        let text = String::from_utf8(stdout).expect("schema should be utf8");
        let parsed: serde_json::Value =
            serde_json::from_str(&text).expect("schema should be valid json");

        assert_eq!(code, std::process::ExitCode::SUCCESS);
        assert!(stderr.is_empty());
        assert_eq!(parsed["title"], "rit status");
        assert!(text.contains("\"$id\": \"https://rit.dev/schemas/v1/status.json\""));
        assert!(text.contains("\"entries\""));
    }

    #[test]
    fn schema_command_rejects_unknown_names() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = schema_command(&["missing".to_owned()], &mut stdout, &mut stderr)
            .expect("schema command should run");
        let error = String::from_utf8(stderr).expect("error should be utf8");

        assert_eq!(code, std::process::ExitCode::from(129));
        assert!(stdout.is_empty());
        assert!(error.contains("unknown schema"));
        assert!(error.contains("status"));
    }
}
