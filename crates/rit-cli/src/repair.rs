use std::io::{self, Write};
use std::process::ExitCode;

pub fn repair_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let Some(apply) = parse_repair_args(args, stderr)? else {
        return Ok(ExitCode::from(129));
    };
    let repository = match rit_core::Repository::discover(".") {
        Ok(repository) => repository,
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            return Ok(ExitCode::from(128));
        }
    };
    let plan = repository.repair_plan();

    if plan.is_empty() {
        writeln!(stdout, "repair: nothing to do")?;
        return Ok(ExitCode::SUCCESS);
    }

    if apply {
        match repository.apply_repair_plan(&plan) {
            Ok(result) => {
                writeln!(stdout, "repair: applied")?;
                for action in result.applied {
                    writeln!(stdout, "apply: {}", action.description())?;
                }
                Ok(ExitCode::SUCCESS)
            }
            Err(error) => {
                writeln!(stderr, "rit: {error}")?;
                Ok(ExitCode::from(1))
            }
        }
    } else {
        writeln!(stdout, "repair: dry-run")?;
        for action in plan.actions {
            writeln!(stdout, "would: {}", action.description())?;
        }
        Ok(ExitCode::SUCCESS)
    }
}

fn parse_repair_args(args: &[String], stderr: &mut dyn Write) -> io::Result<Option<bool>> {
    match args {
        [] => Ok(Some(false)),
        [flag] if flag == "--dry-run" => Ok(Some(false)),
        [flag] if flag == "--apply" => Ok(Some(true)),
        [flag] if flag.starts_with('-') => {
            writeln!(stderr, "rit: unsupported repair option '{flag}'")?;
            Ok(None)
        }
        [unexpected, ..] => {
            writeln!(stderr, "rit: unexpected repair argument '{unexpected}'")?;
            Ok(None)
        }
    }
}
