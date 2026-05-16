use std::io::{self, Write};
use std::process::ExitCode;

pub fn repair_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let Some(repair_args) = parse_repair_args(args, stderr)? else {
        return Ok(ExitCode::from(129));
    };
    #[cfg(not(feature = "indexdb"))]
    if repair_args.drop_corrupt_indexdb {
        writeln!(
            stderr,
            "rit: this build does not include indexdb support; rebuild with the `indexdb` feature"
        )?;
        return Ok(ExitCode::from(1));
    }
    let repository = match rit_core::Repository::discover(".") {
        Ok(repository) => repository,
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            return Ok(ExitCode::from(128));
        }
    };
    #[cfg(feature = "indexdb")]
    let plan = {
        let mut options = rit_core::RepairOptions::new();
        if repair_args.drop_corrupt_indexdb {
            options = options.drop_corrupt_indexdb();
        }
        repository.repair_plan_with_options(options)
    };
    #[cfg(not(feature = "indexdb"))]
    let plan = repository.repair_plan();

    if plan.is_empty() {
        writeln!(stdout, "repair: nothing to do")?;
        return Ok(ExitCode::SUCCESS);
    }

    if repair_args.apply {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RepairArgs {
    apply: bool,
    drop_corrupt_indexdb: bool,
}

fn parse_repair_args(args: &[String], stderr: &mut dyn Write) -> io::Result<Option<RepairArgs>> {
    let mut parsed = RepairArgs {
        apply: false,
        drop_corrupt_indexdb: false,
    };
    let mut saw_mode = false;

    for arg in args {
        match arg.as_str() {
            "--dry-run" => {
                if saw_mode {
                    writeln!(stderr, "rit: choose only one repair mode")?;
                    return Ok(None);
                }
                saw_mode = true;
                parsed.apply = false;
            }
            "--apply" => {
                if saw_mode {
                    writeln!(stderr, "rit: choose only one repair mode")?;
                    return Ok(None);
                }
                saw_mode = true;
                parsed.apply = true;
            }
            "--drop-indexdb" => {
                parsed.drop_corrupt_indexdb = true;
            }
            flag if flag.starts_with('-') => {
                writeln!(stderr, "rit: unsupported repair option '{flag}'")?;
                return Ok(None);
            }
            unexpected => {
                writeln!(stderr, "rit: unexpected repair argument '{unexpected}'")?;
                return Ok(None);
            }
        }
    }

    Ok(Some(parsed))
}
