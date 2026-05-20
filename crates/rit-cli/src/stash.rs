use std::io::{self, Write};
use std::process::ExitCode;

use crate::{
    discover_repository, is_valid_break_rewrites_option, parse_rename_limit_option,
    parse_similarity_option, pathspec_args, pathspec_from_file_missing_value, write_command_error,
    write_pathspec_file_cannot_mix_with_args, write_pathspec_file_nul_requires_file,
    write_pathspec_from_file_requires_value,
};

pub(super) fn stash_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    match args {
        [] => {}
        [flag, ..] if flag.starts_with('-') => {}
        [subcommand] if subcommand == "list" => {}
        [subcommand] if subcommand == "clear" => {}
        [subcommand, ..] if subcommand == "create" => {}
        [subcommand, rest @ ..] if subcommand == "save" => {
            if parse_stash_save_args(rest, stderr)?.is_none() {
                return Ok(ExitCode::from(129));
            }
        }
        [subcommand, rest @ ..] if subcommand == "branch" => {
            if parse_stash_branch_args(rest, stderr)?.is_none() {
                return Ok(ExitCode::from(129));
            }
        }
        [subcommand, rest @ ..] if subcommand == "apply" => {
            if parse_stash_apply_args(rest, stderr)?.is_none() {
                return Ok(ExitCode::from(129));
            }
        }
        [subcommand, rest @ ..] if subcommand == "pop" => {
            if parse_stash_pop_args(rest, stderr)?.is_none() {
                return Ok(ExitCode::from(129));
            }
        }
        [subcommand, ..] if subcommand == "push" => {}
        [subcommand, rest @ ..] if subcommand == "show" => {
            match parse_stash_show_args(rest, stderr)? {
                Some(show_args) => {
                    if let Some(exit_code) = show_args.immediate_exit_code {
                        return Ok(ExitCode::from(exit_code));
                    }
                }
                None => return Ok(ExitCode::from(129)),
            }
        }
        [subcommand, rest @ ..] if subcommand == "drop" => {
            if parse_stash_drop_args(rest, stderr)?.is_none() {
                return Ok(ExitCode::from(129));
            }
        }
        [subcommand, rest @ ..] if subcommand == "store" => {
            if parse_stash_store_args(rest, stderr)?.is_none() {
                return Ok(ExitCode::from(129));
            }
        }
        [subcommand, rest @ ..] if subcommand == "export" => {
            if parse_stash_export_args(rest, stderr)?.is_none() {
                return Ok(ExitCode::from(129));
            }
        }
        [subcommand, rest @ ..] if subcommand == "import" => {
            if parse_stash_import_args(rest, stderr)?.is_none() {
                return Ok(ExitCode::from(129));
            }
        }
        [subcommand, ..] => {
            writeln!(stderr, "rit: unsupported stash subcommand '{subcommand}'")?;
            return Ok(ExitCode::from(129));
        }
    }

    let repository = match discover_repository(stderr)? {
        Some(repository) => repository,
        None => return Ok(ExitCode::from(128)),
    };
    if matches!(args, [subcommand] if subcommand == "clear") {
        return match repository.stash_clear() {
            Ok(()) => Ok(ExitCode::SUCCESS),
            Err(error) => write_command_error(stderr, error),
        };
    }
    if let [subcommand, rest @ ..] = args
        && subcommand == "apply"
    {
        let Some(apply_args) = parse_stash_apply_args(rest, stderr)? else {
            return Ok(ExitCode::from(129));
        };
        return match repository.stash_apply_with_index(apply_args.index, apply_args.restore_index) {
            Ok(_result) => {
                if !apply_args.quiet {
                    write_stash_already_up_to_date_if_no_tracked_changes(&repository, stdout)?;
                    write_stash_human_status(&repository, stdout)?;
                }
                Ok(ExitCode::SUCCESS)
            }
            Err(error) => {
                if apply_args.quiet && is_stash_untracked_restore_error(&error) {
                    writeln!(stdout, "Already up to date.")?;
                }
                write_stash_error(stderr, error)
            }
        };
    }
    if let [subcommand, rest @ ..] = args
        && subcommand == "save"
    {
        let Some(save_args) = parse_stash_save_args(rest, stderr)? else {
            return Ok(ExitCode::from(129));
        };
        let result = if save_args.staged {
            repository.stash_push_staged_with_pathspecs(
                save_args.message.as_deref(),
                &rit_core::PathspecSet::all(),
            )
        } else if save_args.all {
            repository.stash_push_all_with_pathspecs(
                save_args.message.as_deref(),
                &rit_core::PathspecSet::all(),
            )
        } else if save_args.include_untracked {
            repository.stash_push_include_untracked_with_pathspecs(
                save_args.message.as_deref(),
                &rit_core::PathspecSet::all(),
            )
        } else if save_args.keep_index {
            repository.stash_push_keep_index_with_pathspecs(
                save_args.message.as_deref(),
                &rit_core::PathspecSet::all(),
            )
        } else {
            repository.stash_push(save_args.message.as_deref())
        };
        return match result {
            Ok(rit_core::StashPushResult::NoLocalChanges) => {
                if !save_args.quiet {
                    writeln!(stdout, "No local changes to save")?;
                }
                Ok(ExitCode::SUCCESS)
            }
            Ok(rit_core::StashPushResult::Saved { message, .. }) => {
                if !save_args.quiet {
                    writeln!(stdout, "Saved working directory and index state {message}")?;
                }
                Ok(ExitCode::SUCCESS)
            }
            Ok(rit_core::StashPushResult::SavedCleanupFailed {
                message,
                cleanup_error,
                ..
            }) => {
                if !save_args.quiet {
                    writeln!(stdout, "Saved working directory and index state {message}")?;
                }
                writeln!(stderr, "{cleanup_error}")?;
                Ok(ExitCode::from(1))
            }
            Err(error) => write_command_error(stderr, error),
        };
    }
    if let [subcommand, rest @ ..] = args
        && subcommand == "branch"
    {
        let Some(branch_args) = parse_stash_branch_args(rest, stderr)? else {
            return Ok(ExitCode::from(129));
        };
        return match repository.stash_branch(
            &branch_args.branch,
            branch_args.index,
            branch_args.name.clone(),
        ) {
            Ok(result) => {
                writeln!(stderr, "Switched to a new branch '{}'", branch_args.branch)?;
                write_stash_already_up_to_date_if_no_tracked_changes(&repository, stdout)?;
                write_stash_human_status(&repository, stdout)?;
                writeln!(stdout, "Dropped {} ({})", result.name, result.object_id)?;
                Ok(ExitCode::SUCCESS)
            }
            Err(error) => write_stash_error(stderr, error),
        };
    }
    if let [subcommand, rest @ ..] = args
        && subcommand == "pop"
    {
        let Some(pop_args) = parse_stash_pop_args(rest, stderr)? else {
            return Ok(ExitCode::from(129));
        };
        return match repository.stash_pop_with_index(
            pop_args.index,
            pop_args.name.clone(),
            pop_args.restore_index,
        ) {
            Ok(result) => {
                if !pop_args.quiet {
                    write_stash_already_up_to_date_if_no_tracked_changes(&repository, stdout)?;
                    write_stash_human_status(&repository, stdout)?;
                }
                if !pop_args.quiet {
                    writeln!(stdout, "Dropped {} ({})", result.name, result.object_id)?;
                }
                Ok(ExitCode::SUCCESS)
            }
            Err(error) => {
                if pop_args.quiet && is_stash_untracked_restore_error(&error) {
                    writeln!(stdout, "Already up to date.")?;
                    writeln!(stdout, "The stash entry is kept in case you need it again.")?;
                }
                write_stash_error(stderr, error)
            }
        };
    }
    if let [subcommand, rest @ ..] = args
        && subcommand == "create"
    {
        let message = (!rest.is_empty()).then(|| rest.join(" "));
        return match repository.stash_create(message.as_deref()) {
            Ok(Some(object_id)) => {
                writeln!(stdout, "{object_id}")?;
                Ok(ExitCode::SUCCESS)
            }
            Ok(None) => Ok(ExitCode::SUCCESS),
            Err(error) => write_command_error(stderr, error),
        };
    }
    if args.is_empty()
        || matches!(args, [subcommand, ..] if subcommand == "push")
        || matches!(args, [flag, ..] if flag.starts_with('-'))
    {
        let push_args = if let [subcommand, rest @ ..] = args {
            let push_args = if subcommand == "push" { rest } else { args };
            let Some(parsed) = parse_stash_push_args(push_args, stderr)? else {
                return Ok(ExitCode::from(129));
            };
            parsed
        } else {
            StashPushArgs::default()
        };
        if let Some(exit_code) = push_args.exit_code {
            return Ok(ExitCode::from(exit_code));
        }
        let pathspecs = match rit_core::PathspecSet::from_args(&push_args.pathspecs) {
            Ok(pathspecs) => pathspecs,
            Err(error) => return write_command_error(stderr, error),
        };
        let result = if push_args.staged {
            repository.stash_push_staged_with_pathspecs(push_args.message.as_deref(), &pathspecs)
        } else if push_args.all {
            repository.stash_push_all_with_pathspecs(push_args.message.as_deref(), &pathspecs)
        } else if push_args.include_untracked {
            repository.stash_push_include_untracked_with_pathspecs(
                push_args.message.as_deref(),
                &pathspecs,
            )
        } else if push_args.keep_index {
            repository
                .stash_push_keep_index_with_pathspecs(push_args.message.as_deref(), &pathspecs)
        } else {
            repository.stash_push_with_pathspecs(push_args.message.as_deref(), &pathspecs)
        };
        return match result {
            Ok(rit_core::StashPushResult::NoLocalChanges) => {
                if !push_args.quiet {
                    writeln!(stdout, "No local changes to save")?;
                }
                Ok(ExitCode::SUCCESS)
            }
            Ok(rit_core::StashPushResult::Saved { message, .. }) => {
                if !push_args.quiet {
                    writeln!(stdout, "Saved working directory and index state {message}")?;
                }
                Ok(ExitCode::SUCCESS)
            }
            Ok(rit_core::StashPushResult::SavedCleanupFailed {
                message,
                cleanup_error,
                ..
            }) => {
                if !push_args.quiet {
                    writeln!(stdout, "Saved working directory and index state {message}")?;
                }
                writeln!(stderr, "{cleanup_error}")?;
                Ok(ExitCode::from(1))
            }
            Err(error) => write_command_error(stderr, error),
        };
    }
    if let [subcommand, rest @ ..] = args
        && subcommand == "show"
    {
        let Some(show_args) = parse_stash_show_args(rest, stderr)? else {
            return Ok(ExitCode::from(129));
        };
        if let Some(exit_code) = show_args.immediate_exit_code {
            return Ok(ExitCode::from(exit_code));
        }
        let mut output_file;
        let stdout: &mut dyn Write = if let Some(path) = &show_args.output_path {
            output_file = std::fs::File::create(path)?;
            &mut output_file
        } else {
            stdout
        };
        let format = match stash_show_format(
            &repository,
            show_args.format,
            show_args.exit_code || show_args.diff_option,
        ) {
            Ok(format) => format,
            Err(error) => return write_command_error(stderr, error),
        };
        let untracked_mode = match stash_show_untracked_mode(&repository, show_args.untracked_mode)
        {
            Ok(mode) => mode,
            Err(error) => return write_command_error(stderr, error),
        };
        if matches!(
            format,
            StashShowFormat::Patch
                | StashShowFormat::StatAndPatch
                | StashShowFormat::Raw
                | StashShowFormat::RawAndPatch
                | StashShowFormat::Summary
                | StashShowFormat::SummaryAndPatch
                | StashShowFormat::StatAndSummary
                | StashShowFormat::CompactStatAndPatch
                | StashShowFormat::CompactStatAndSummary
        ) {
            let patch_result = match untracked_mode {
                StashShowUntrackedMode::Tracked => {
                    repository.stash_show_patch(show_args.index, &rit_core::PathspecSet::all())
                }
                StashShowUntrackedMode::Include => repository.stash_show_patch_include_untracked(
                    show_args.index,
                    &rit_core::PathspecSet::all(),
                ),
                StashShowUntrackedMode::Only => repository.stash_show_patch_only_untracked(
                    show_args.index,
                    &rit_core::PathspecSet::all(),
                ),
            };
            return match patch_result {
                Ok(patch) => {
                    let patch = match &show_args.diff_filter {
                        Some(filter) => patch.into_filtered_by_status(filter),
                        None => patch,
                    };
                    let patch = match &show_args.relative_path {
                        Some(path) => patch.into_relative_to_path(path),
                        None => patch,
                    };
                    let patch_options = stash_show_patch_options(&show_args);
                    match patch.to_patch_text_with_options(&patch_options) {
                        Ok(text) => {
                            let has_changes = !patch.files.is_empty();
                            if matches!(format, StashShowFormat::Raw | StashShowFormat::RawAndPatch)
                            {
                                let raw_text = patch.to_raw_text_with_options(&patch_options);
                                write_stash_show_text(stdout, &show_args.line_prefix, &raw_text)?;
                                if matches!(format, StashShowFormat::Raw) {
                                    return Ok(stash_show_exit_code(
                                        show_args.exit_code,
                                        has_changes,
                                    ));
                                }
                                if !raw_text.is_empty() && !text.is_empty() {
                                    write_stash_show_blank_line(stdout, &show_args.line_prefix)?;
                                }
                            }
                            if matches!(
                                format,
                                StashShowFormat::Summary | StashShowFormat::SummaryAndPatch
                            ) {
                                let summary_text = patch.to_summary_text();
                                write_stash_show_text(
                                    stdout,
                                    &show_args.line_prefix,
                                    &summary_text,
                                )?;
                                if matches!(format, StashShowFormat::Summary) {
                                    return Ok(stash_show_exit_code(
                                        show_args.exit_code,
                                        has_changes,
                                    ));
                                }
                                if !summary_text.is_empty() && !text.is_empty() {
                                    write_stash_show_blank_line(stdout, &show_args.line_prefix)?;
                                }
                            }
                            if matches!(
                                format,
                                StashShowFormat::StatAndSummary
                                    | StashShowFormat::CompactStatAndSummary
                            ) {
                                let summary = match untracked_mode {
                                    StashShowUntrackedMode::Tracked => repository
                                        .stash_show(show_args.index, &rit_core::PathspecSet::all()),
                                    StashShowUntrackedMode::Include => repository
                                        .stash_show_include_untracked(
                                            show_args.index,
                                            &rit_core::PathspecSet::all(),
                                        ),
                                    StashShowUntrackedMode::Only => repository
                                        .stash_show_only_untracked(
                                            show_args.index,
                                            &rit_core::PathspecSet::all(),
                                        ),
                                };
                                match summary {
                                    Ok(diff) => {
                                        let diff = match &show_args.diff_filter {
                                            Some(filter) => diff.into_filtered_by_status(filter),
                                            None => diff,
                                        };
                                        let diff = match &show_args.relative_path {
                                            Some(path) => diff.into_relative_to_path(path),
                                            None => diff,
                                        };
                                        let stat_text = if matches!(
                                            format,
                                            StashShowFormat::CompactStatAndSummary
                                        ) {
                                            diff.to_compact_stat_text()
                                        } else {
                                            diff.to_stat_text()
                                        };
                                        write_stash_show_text(
                                            stdout,
                                            &show_args.line_prefix,
                                            &stat_text,
                                        )?;
                                        write_stash_show_text(
                                            stdout,
                                            &show_args.line_prefix,
                                            &patch.to_summary_text(),
                                        )?;
                                        return Ok(stash_show_exit_code(
                                            show_args.exit_code,
                                            has_changes,
                                        ));
                                    }
                                    Err(error) => return write_stash_error(stderr, error),
                                }
                            }
                            if matches!(
                                format,
                                StashShowFormat::StatAndPatch
                                    | StashShowFormat::CompactStatAndPatch
                            ) {
                                let summary = match untracked_mode {
                                    StashShowUntrackedMode::Tracked => repository
                                        .stash_show(show_args.index, &rit_core::PathspecSet::all()),
                                    StashShowUntrackedMode::Include => repository
                                        .stash_show_include_untracked(
                                            show_args.index,
                                            &rit_core::PathspecSet::all(),
                                        ),
                                    StashShowUntrackedMode::Only => repository
                                        .stash_show_only_untracked(
                                            show_args.index,
                                            &rit_core::PathspecSet::all(),
                                        ),
                                };
                                match summary {
                                    Ok(diff) => {
                                        let diff = match &show_args.diff_filter {
                                            Some(filter) => diff.into_filtered_by_status(filter),
                                            None => diff,
                                        };
                                        let diff = match &show_args.relative_path {
                                            Some(path) => diff.into_relative_to_path(path),
                                            None => diff,
                                        };
                                        let stat_text = if matches!(
                                            format,
                                            StashShowFormat::CompactStatAndPatch
                                        ) {
                                            diff.to_compact_stat_text()
                                        } else {
                                            diff.to_stat_text()
                                        };
                                        write_stash_show_text(
                                            stdout,
                                            &show_args.line_prefix,
                                            &stat_text,
                                        )?;
                                        if !stat_text.is_empty() && !text.is_empty() {
                                            write_stash_show_blank_line(
                                                stdout,
                                                &show_args.line_prefix,
                                            )?;
                                        }
                                    }
                                    Err(error) => return write_stash_error(stderr, error),
                                }
                            }
                            write_stash_show_text(stdout, &show_args.line_prefix, &text)?;
                            Ok(stash_show_exit_code(show_args.exit_code, has_changes))
                        }
                        Err(error) => write_command_error(stderr, error),
                    }
                }
                Err(error) => write_stash_error(stderr, error),
            };
        }
        let summary = match untracked_mode {
            StashShowUntrackedMode::Tracked => {
                repository.stash_show(show_args.index, &rit_core::PathspecSet::all())
            }
            StashShowUntrackedMode::Include => repository
                .stash_show_include_untracked(show_args.index, &rit_core::PathspecSet::all()),
            StashShowUntrackedMode::Only => {
                repository.stash_show_only_untracked(show_args.index, &rit_core::PathspecSet::all())
            }
        };
        return match summary {
            Ok(diff) => {
                let diff = match &show_args.diff_filter {
                    Some(filter) => diff.into_filtered_by_status(filter),
                    None => diff,
                };
                let diff = match &show_args.relative_path {
                    Some(path) => diff.into_relative_to_path(path),
                    None => diff,
                };
                match format {
                    StashShowFormat::None => {}
                    StashShowFormat::Quiet => {
                        return Ok(stash_show_exit_code(true, !diff.files.is_empty()));
                    }
                    StashShowFormat::Stat => {
                        write_stash_show_text(stdout, &show_args.line_prefix, &diff.to_stat_text())?
                    }
                    StashShowFormat::CompactStat => write_stash_show_text(
                        stdout,
                        &show_args.line_prefix,
                        &diff.to_compact_stat_text(),
                    )?,
                    StashShowFormat::Patch => {
                        unreachable!("patch is handled before summary output")
                    }
                    StashShowFormat::StatAndPatch => {
                        unreachable!("stat and patch are handled before summary output")
                    }
                    StashShowFormat::CompactStatAndPatch => {
                        unreachable!("compact stat and patch are handled before summary output")
                    }
                    StashShowFormat::Raw | StashShowFormat::RawAndPatch => {
                        unreachable!("raw is handled before summary output")
                    }
                    StashShowFormat::Summary | StashShowFormat::SummaryAndPatch => {
                        unreachable!("summary is handled before summary output")
                    }
                    StashShowFormat::StatAndSummary => {
                        unreachable!("stat and summary are handled before summary output")
                    }
                    StashShowFormat::CompactStatAndSummary => {
                        unreachable!("compact stat and summary are handled before summary output")
                    }
                    StashShowFormat::ShortStat => write_stash_show_text(
                        stdout,
                        &show_args.line_prefix,
                        &diff.to_shortstat_text(),
                    )?,
                    StashShowFormat::NameOnly => {
                        if show_args.nul_terminated {
                            stdout.write_all(&diff.to_name_only_z())?;
                        } else {
                            let mut text = String::new();
                            for path in diff.name_only() {
                                text.push_str(path);
                                text.push('\n');
                            }
                            write_stash_show_text(stdout, &show_args.line_prefix, &text)?;
                        }
                    }
                    StashShowFormat::NameStatus => {
                        if show_args.nul_terminated {
                            stdout.write_all(&diff.to_name_status_z())?;
                        } else {
                            write_stash_show_text(
                                stdout,
                                &show_args.line_prefix,
                                &diff.to_name_status_text(),
                            )?;
                        }
                    }
                    StashShowFormat::Numstat => {
                        if show_args.nul_terminated {
                            stdout.write_all(&diff.to_numstat_z())?
                        } else {
                            write_stash_show_text(
                                stdout,
                                &show_args.line_prefix,
                                &diff.to_numstat_text(),
                            )?
                        }
                    }
                }
                Ok(stash_show_exit_code(
                    show_args.exit_code,
                    !diff.files.is_empty(),
                ))
            }
            Err(error) => write_stash_error(stderr, error),
        };
    }
    if let [subcommand, rest @ ..] = args
        && subcommand == "drop"
    {
        let Some(drop_args) = parse_stash_drop_args(rest, stderr)? else {
            return Ok(ExitCode::from(129));
        };
        return match repository.stash_drop(drop_args.index, drop_args.name.clone()) {
            Ok(result) => {
                if !drop_args.quiet {
                    writeln!(stdout, "Dropped {} ({})", result.name, result.object_id)?;
                }
                Ok(ExitCode::SUCCESS)
            }
            Err(error) => write_stash_error(stderr, error),
        };
    }
    if let [subcommand, rest @ ..] = args
        && subcommand == "store"
    {
        let Some(store_args) = parse_stash_store_args(rest, stderr)? else {
            return Ok(ExitCode::from(129));
        };
        let target = match repository.resolve_revision(&store_args.commit) {
            Ok(target) => target,
            Err(error) => return write_command_error(stderr, error),
        };
        return match repository.stash_store(target, store_args.message.as_deref()) {
            Ok(()) => Ok(ExitCode::SUCCESS),
            Err(error) => write_command_error(stderr, error),
        };
    }
    if let [subcommand, rest @ ..] = args
        && subcommand == "export"
    {
        let Some(export_args) = parse_stash_export_args(rest, stderr)? else {
            return Ok(ExitCode::from(129));
        };
        return match export_args.target {
            StashExportTarget::Print => match repository.stash_export(&export_args.indices) {
                Ok(object_id) => {
                    writeln!(stdout, "{object_id}")?;
                    Ok(ExitCode::SUCCESS)
                }
                Err(error) => write_stash_error(stderr, error),
            },
            StashExportTarget::ToRef(ref_name) => {
                match repository.stash_export_to_ref(&export_args.indices, &ref_name) {
                    Ok(_) => Ok(ExitCode::SUCCESS),
                    Err(error) => write_stash_error(stderr, error),
                }
            }
        };
    }
    if let [subcommand, rest @ ..] = args
        && subcommand == "import"
    {
        let Some(import_args) = parse_stash_import_args(rest, stderr)? else {
            return Ok(ExitCode::from(129));
        };
        let target = match repository.resolve_revision(&import_args.commit) {
            Ok(target) => target,
            Err(error) => return write_command_error(stderr, error),
        };
        return match repository.stash_import(target) {
            Ok(_) => Ok(ExitCode::SUCCESS),
            Err(error) => write_stash_error(stderr, error),
        };
    }

    match repository.stash_list() {
        Ok(entries) => {
            for entry in entries {
                writeln!(stdout, "stash@{{{}}}: {}", entry.index, entry.message)?;
            }
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => write_command_error(stderr, error),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StashShowFormat {
    None,
    Quiet,
    Stat,
    CompactStat,
    Patch,
    StatAndPatch,
    CompactStatAndPatch,
    Raw,
    RawAndPatch,
    Summary,
    SummaryAndPatch,
    StatAndSummary,
    CompactStatAndSummary,
    ShortStat,
    NameOnly,
    NameStatus,
    Numstat,
}

struct StashShowArgs {
    index: usize,
    immediate_exit_code: Option<u8>,
    format: Option<StashShowFormat>,
    untracked_mode: Option<StashShowUntrackedMode>,
    exit_code: bool,
    diff_option: bool,
    nul_terminated: bool,
    full_index: bool,
    abbrev: usize,
    context_lines: usize,
    inter_hunk_context: usize,
    default_prefixes: bool,
    old_path_prefix: String,
    new_path_prefix: String,
    line_prefix: String,
    relative_path: Option<String>,
    new_line_indicator: Option<char>,
    old_line_indicator: Option<char>,
    context_line_indicator: Option<char>,
    diff_filter: Option<rit_core::DiffStatusFilter>,
    output_path: Option<String>,
}

impl StashShowArgs {
    fn immediate_exit(exit_code: u8) -> Self {
        Self {
            index: 0,
            immediate_exit_code: Some(exit_code),
            format: None,
            untracked_mode: None,
            exit_code: false,
            diff_option: false,
            nul_terminated: false,
            full_index: false,
            abbrev: 7,
            context_lines: 3,
            inter_hunk_context: 0,
            default_prefixes: true,
            old_path_prefix: "a/".to_owned(),
            new_path_prefix: "b/".to_owned(),
            line_prefix: String::new(),
            relative_path: None,
            new_line_indicator: Some('+'),
            old_line_indicator: Some('-'),
            context_line_indicator: Some(' '),
            diff_filter: None,
            output_path: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StashShowUntrackedMode {
    Tracked,
    Include,
    Only,
}

fn parse_stash_show_args(
    args: &[String],
    stderr: &mut dyn Write,
) -> io::Result<Option<StashShowArgs>> {
    let mut format = None;
    let mut untracked_mode = None;
    let mut exit_code = false;
    let mut diff_option = false;
    let mut nul_terminated = false;
    let mut full_index = false;
    let mut abbrev = 7;
    let mut context_lines = 3;
    let mut inter_hunk_context = 0;
    let mut default_prefixes = true;
    let mut old_path_prefix = "a/".to_owned();
    let mut new_path_prefix = "b/".to_owned();
    let mut line_prefix = String::new();
    let mut relative_path = None;
    let mut new_line_indicator = Some('+');
    let mut old_line_indicator = Some('-');
    let mut context_line_indicator = Some(' ');
    let mut diff_filter = None;
    let mut output_path = None;
    let mut stash = None;
    for arg in args {
        match arg.as_str() {
            "--stat" => {
                format = Some(match format {
                    Some(StashShowFormat::Patch) => StashShowFormat::StatAndPatch,
                    Some(StashShowFormat::Summary) => StashShowFormat::StatAndSummary,
                    Some(StashShowFormat::CompactStat) => StashShowFormat::Stat,
                    Some(StashShowFormat::CompactStatAndPatch) => StashShowFormat::StatAndPatch,
                    Some(StashShowFormat::CompactStatAndSummary) => StashShowFormat::StatAndSummary,
                    _ => StashShowFormat::Stat,
                });
            }
            "--compact-summary" => {
                format = Some(match format {
                    Some(StashShowFormat::Patch) => StashShowFormat::CompactStatAndPatch,
                    Some(StashShowFormat::Summary) => StashShowFormat::CompactStatAndSummary,
                    Some(StashShowFormat::StatAndPatch) => StashShowFormat::CompactStatAndPatch,
                    Some(StashShowFormat::StatAndSummary) => StashShowFormat::CompactStatAndSummary,
                    _ => StashShowFormat::CompactStat,
                });
            }
            "--no-compact-summary" => {
                diff_option = true;
                format = match format {
                    Some(StashShowFormat::CompactStat) => Some(StashShowFormat::Stat),
                    Some(StashShowFormat::CompactStatAndPatch) => {
                        Some(StashShowFormat::StatAndPatch)
                    }
                    Some(StashShowFormat::CompactStatAndSummary) => {
                        Some(StashShowFormat::StatAndSummary)
                    }
                    other => other,
                };
            }
            "--shortstat" => format = Some(StashShowFormat::ShortStat),
            "--quiet" => format = Some(StashShowFormat::Quiet),
            "--exit-code" => exit_code = true,
            "-z" => {
                diff_option = true;
                nul_terminated = true;
            }
            "--full-index" => {
                diff_option = true;
                full_index = true;
            }
            "--abbrev" => {
                diff_option = true;
                abbrev = 7;
            }
            "--no-ext-diff"
            | "--ext-diff"
            | "--no-color"
            | "--color=never"
            | "--color=auto"
            | "--color-moved"
            | "--no-color-moved"
            | "--no-color-moved-ws"
            | "--relative"
            | "--no-relative"
            | "--binary"
            | "--no-renames"
            | "--find-renames"
            | "--find-copies"
            | "--find-copies-harder"
            | "--pickaxe-all"
            | "--pickaxe-regex"
            | "--break-rewrites"
            | "-B"
            | "-M"
            | "-C"
            | "--minimal"
            | "--patience"
            | "--histogram"
            | "-w"
            | "--ignore-all-space"
            | "-b"
            | "--ignore-space-change"
            | "--ignore-space-at-eol"
            | "--ignore-cr-at-eol"
            | "--ignore-blank-lines"
            | "--indent-heuristic"
            | "--no-indent-heuristic"
            | "--irreversible-delete"
            | "-D"
            | "--function-context"
            | "-W"
            | "-a"
            | "--text"
            | "--textconv"
            | "--no-textconv"
            | "--ignore-submodules"
            | "--submodule"
            | "--ita-invisible-in-index"
            | "--ita-visible-in-index" => diff_option = true,
            "--no-prefix" => {
                diff_option = true;
                default_prefixes = false;
            }
            "--default-prefix" => {
                diff_option = true;
                default_prefixes = true;
            }
            "--patch-with-stat" => format = Some(StashShowFormat::StatAndPatch),
            "--patch-with-raw" => format = Some(StashShowFormat::RawAndPatch),
            "-p" | "--patch" => {
                format = Some(match format {
                    Some(StashShowFormat::Raw) => StashShowFormat::RawAndPatch,
                    Some(StashShowFormat::Summary) => StashShowFormat::SummaryAndPatch,
                    Some(StashShowFormat::Stat) => StashShowFormat::StatAndPatch,
                    Some(StashShowFormat::CompactStat) => StashShowFormat::CompactStatAndPatch,
                    _ => StashShowFormat::Patch,
                });
            }
            "--no-patch" => format = Some(StashShowFormat::None),
            "--raw" => {
                format = Some(match format {
                    Some(StashShowFormat::Patch) => StashShowFormat::RawAndPatch,
                    _ => StashShowFormat::Raw,
                });
            }
            "--summary" => {
                format = Some(match format {
                    Some(StashShowFormat::Patch) => StashShowFormat::SummaryAndPatch,
                    Some(StashShowFormat::Stat) => StashShowFormat::StatAndSummary,
                    Some(StashShowFormat::CompactStat) => StashShowFormat::CompactStatAndSummary,
                    Some(StashShowFormat::CompactStatAndPatch) => {
                        StashShowFormat::CompactStatAndSummary
                    }
                    _ => StashShowFormat::Summary,
                });
            }
            "--name-only" => format = Some(StashShowFormat::NameOnly),
            "--name-status" => format = Some(StashShowFormat::NameStatus),
            "--numstat" => format = Some(StashShowFormat::Numstat),
            "-u" | "--include-untracked" => untracked_mode = Some(StashShowUntrackedMode::Include),
            "--no-include-untracked" => untracked_mode = Some(StashShowUntrackedMode::Tracked),
            "--only-untracked" => untracked_mode = Some(StashShowUntrackedMode::Only),
            _ if arg.starts_with("--abbrev=") => {
                let value = arg.trim_start_matches("--abbrev=");
                diff_option = true;
                abbrev = value.parse::<usize>().unwrap_or(0).max(4);
            }
            _ if arg.starts_with("--unified=") => {
                let value = arg.trim_start_matches("--unified=");
                diff_option = true;
                context_lines = match parse_diff_context_lines(value) {
                    Some(context_lines) => context_lines,
                    None => {
                        writeln!(stderr, "error: --unified expects a numerical value")?;
                        return Ok(None);
                    }
                };
            }
            _ if arg.starts_with("--inter-hunk-context=") => {
                let value = arg.trim_start_matches("--inter-hunk-context=");
                diff_option = true;
                inter_hunk_context = match parse_inter_hunk_context_lines(value) {
                    Some(inter_hunk_context) => inter_hunk_context,
                    None => {
                        writeln!(
                            stderr,
                            "error: option `inter-hunk-context' expects an integer value with an optional k/m/g suffix"
                        )?;
                        return Ok(None);
                    }
                };
            }
            _ if arg.starts_with("-U") && arg.len() > 2 => {
                let value = arg.trim_start_matches("-U");
                diff_option = true;
                context_lines = match parse_diff_context_lines(value) {
                    Some(context_lines) => context_lines,
                    None => {
                        writeln!(stderr, "error: --unified expects a numerical value")?;
                        return Ok(None);
                    }
                };
            }
            _ if arg.starts_with("--diff-filter=") => {
                let value = arg.trim_start_matches("--diff-filter=");
                diff_option = true;
                match rit_core::DiffStatusFilter::from_git_diff_filter(value) {
                    Ok(filter) => diff_filter = Some(filter),
                    Err(error) => {
                        writeln!(stderr, "error: {error}")?;
                        return Ok(None);
                    }
                }
            }
            _ if arg.starts_with("--diff-algorithm=") => {
                let value = arg.trim_start_matches("--diff-algorithm=");
                diff_option = true;
                if !matches!(
                    value,
                    "myers" | "minimal" | "patience" | "histogram" | "default"
                ) {
                    writeln!(
                        stderr,
                        "error: option diff-algorithm accepts \"myers\", \"minimal\", \"patience\" and \"histogram\""
                    )?;
                    return Ok(None);
                }
            }
            _ if arg.starts_with("--word-diff=") => {
                let value = arg.trim_start_matches("--word-diff=");
                diff_option = true;
                if !matches!(value, "none") {
                    writeln!(stderr, "error: bad --word-diff argument: {value}")?;
                    return Ok(None);
                }
            }
            _ if arg.starts_with("--anchored=") => {
                diff_option = true;
            }
            _ if arg.starts_with("--stat=") => {
                let value = arg.trim_start_matches("--stat=");
                diff_option = true;
                format = Some(match format {
                    Some(StashShowFormat::Patch) => StashShowFormat::StatAndPatch,
                    other => other.unwrap_or(StashShowFormat::Stat),
                });
                if !is_valid_stat_value(value) {
                    writeln!(stderr, "error: invalid --stat value: {value}")?;
                    return Ok(None);
                }
            }
            _ if arg.starts_with("--stat-width=")
                || arg.starts_with("--stat-name-width=")
                || arg.starts_with("--stat-graph-width=")
                || arg.starts_with("--stat-count=") =>
            {
                let (option, value) = arg.split_once('=').expect("stat option has '='");
                diff_option = true;
                format = Some(match format {
                    Some(StashShowFormat::Patch) => StashShowFormat::StatAndPatch,
                    other => other.unwrap_or(StashShowFormat::Stat),
                });
                if value.parse::<i64>().is_err() {
                    let option = option.trim_start_matches("--");
                    writeln!(stderr, "error: {option} expects a numerical value")?;
                    return Ok(None);
                }
            }
            _ if arg.starts_with("--src-prefix=") => {
                diff_option = true;
                old_path_prefix = arg.trim_start_matches("--src-prefix=").to_owned();
            }
            _ if arg.starts_with("--dst-prefix=") => {
                diff_option = true;
                new_path_prefix = arg.trim_start_matches("--dst-prefix=").to_owned();
            }
            _ if arg.starts_with("--line-prefix=") => {
                diff_option = true;
                line_prefix = arg.trim_start_matches("--line-prefix=").to_owned();
            }
            _ if arg.starts_with("--relative=") => {
                diff_option = true;
                relative_path = Some(arg.trim_start_matches("--relative=").to_owned());
            }
            _ if arg.starts_with("--output=") => {
                diff_option = true;
                output_path = Some(arg.trim_start_matches("--output=").to_owned());
            }
            _ if arg.starts_with("--color-moved=") => {
                let value = arg.trim_start_matches("--color-moved=");
                diff_option = true;
                if !is_supported_color_moved_mode(value) {
                    writeln!(
                        stderr,
                        "error: color moved setting must be one of 'no', 'default', 'blocks', 'zebra', 'dimmed-zebra', 'plain'"
                    )?;
                    writeln!(stderr, "error: bad --color-moved argument: {value}")?;
                    return Ok(None);
                }
            }
            _ if arg.starts_with("--color-moved-ws=") => {
                let value = arg.trim_start_matches("--color-moved-ws=");
                diff_option = true;
                if let Some(invalid_mode) = first_invalid_color_moved_ws_mode(value) {
                    writeln!(
                        stderr,
                        "error: unknown color-moved-ws mode '{invalid_mode}', possible values are 'ignore-space-change', 'ignore-space-at-eol', 'ignore-all-space', 'allow-indentation-change'"
                    )?;
                    writeln!(
                        stderr,
                        "error: invalid mode '{invalid_mode}' in --color-moved-ws"
                    )?;
                    return Ok(None);
                }
            }
            _ if arg.starts_with("--ws-error-highlight=") => {
                let value = arg.trim_start_matches("--ws-error-highlight=");
                diff_option = true;
                if !is_valid_ws_error_highlight(value) {
                    writeln!(stderr, "error: unknown value after ws-error-highlight=")?;
                    return Ok(None);
                }
            }
            option if option.starts_with("--find-renames=") || option.starts_with("-M") => {
                diff_option = true;
                if let Err(error) = parse_similarity_option(option, "-M", "--find-renames=") {
                    writeln!(stderr, "rit: {error}")?;
                    return Ok(None);
                }
            }
            option if option.starts_with("--find-copies=") || option.starts_with("-C") => {
                diff_option = true;
                if let Err(error) = parse_similarity_option(option, "-C", "--find-copies=") {
                    writeln!(stderr, "rit: {error}")?;
                    return Ok(None);
                }
            }
            option if option.starts_with("--break-rewrites=") || option.starts_with("-B") => {
                diff_option = true;
                if !is_valid_break_rewrites_option(option) {
                    writeln!(stderr, "error: break-rewrites expects <n>/<m> form")?;
                    return Ok(None);
                }
            }
            option if option.starts_with("-l") => {
                diff_option = true;
                if let Err(error) = parse_rename_limit_option(option) {
                    writeln!(stderr, "rit: {error}")?;
                    return Ok(None);
                }
            }
            _ if arg.starts_with("--ignore-submodules=") => {
                let value = arg.trim_start_matches("--ignore-submodules=");
                diff_option = true;
                if !matches!(value, "all" | "none" | "dirty" | "untracked") {
                    writeln!(stderr, "fatal: bad --ignore-submodules argument: {value}")?;
                    return Ok(Some(StashShowArgs::immediate_exit(128)));
                }
            }
            _ if arg.starts_with("--submodule=") => {
                let value = arg.trim_start_matches("--submodule=");
                diff_option = true;
                if !matches!(value, "short" | "log" | "diff") {
                    writeln!(
                        stderr,
                        "error: failed to parse --submodule option parameter: '{value}'"
                    )?;
                    return Ok(None);
                }
            }
            _ if arg.starts_with("--output-indicator-new=") => {
                let value = arg.trim_start_matches("--output-indicator-new=");
                diff_option = true;
                new_line_indicator = match parse_output_indicator(value) {
                    Ok(indicator) => indicator,
                    Err(()) => {
                        writeln!(
                            stderr,
                            "error: output-indicator-new expects a character, got '{value}'"
                        )?;
                        return Ok(None);
                    }
                };
            }
            _ if arg.starts_with("--output-indicator-old=") => {
                let value = arg.trim_start_matches("--output-indicator-old=");
                diff_option = true;
                old_line_indicator = match parse_output_indicator(value) {
                    Ok(indicator) => indicator,
                    Err(()) => {
                        writeln!(
                            stderr,
                            "error: output-indicator-old expects a character, got '{value}'"
                        )?;
                        return Ok(None);
                    }
                };
            }
            _ if arg.starts_with("--output-indicator-context=") => {
                let value = arg.trim_start_matches("--output-indicator-context=");
                diff_option = true;
                context_line_indicator = match parse_output_indicator(value) {
                    Ok(indicator) => indicator,
                    Err(()) => {
                        writeln!(
                            stderr,
                            "error: output-indicator-context expects a character, got '{value}'"
                        )?;
                        return Ok(None);
                    }
                };
            }
            _ if arg.starts_with('-') => {
                writeln!(stderr, "rit: unsupported stash show option '{arg}'")?;
                return Ok(None);
            }
            _ if stash.is_none() => stash = Some(arg.as_str()),
            _ => {
                writeln!(stderr, "rit: stash show accepts at most one stash")?;
                return Ok(None);
            }
        }
    }

    let (index, _) = parse_stash_name(stash.unwrap_or("refs/stash@{0}"))?;
    Ok(Some(StashShowArgs {
        index,
        immediate_exit_code: None,
        format,
        untracked_mode,
        exit_code,
        diff_option,
        nul_terminated,
        full_index,
        abbrev,
        context_lines,
        inter_hunk_context,
        default_prefixes,
        old_path_prefix,
        new_path_prefix,
        line_prefix,
        relative_path,
        new_line_indicator,
        old_line_indicator,
        context_line_indicator,
        diff_filter,
        output_path,
    }))
}

fn parse_diff_context_lines(value: &str) -> Option<usize> {
    value.parse::<usize>().ok()
}

fn parse_inter_hunk_context_lines(value: &str) -> Option<usize> {
    let (digits, multiplier) = match value.as_bytes().last().copied() {
        Some(b'k') => (&value[..value.len() - 1], 1024usize),
        Some(b'm') => (&value[..value.len() - 1], 1024usize * 1024),
        Some(b'g') => (&value[..value.len() - 1], 1024usize * 1024 * 1024),
        _ => (value, 1),
    };
    digits.parse::<usize>().ok()?.checked_mul(multiplier)
}

fn is_valid_stat_value(value: &str) -> bool {
    let fields: Vec<&str> = value.split(',').collect();
    if fields.len() > 3 {
        return false;
    }
    fields
        .iter()
        .all(|field| field.is_empty() || field.parse::<i64>().is_ok())
}

fn parse_output_indicator(value: &str) -> std::result::Result<Option<char>, ()> {
    let mut characters = value.chars();
    let Some(character) = characters.next() else {
        return Ok(None);
    };
    if characters.next().is_some() {
        return Err(());
    }
    Ok(Some(character))
}

fn is_supported_color_moved_mode(value: &str) -> bool {
    matches!(
        value,
        "no" | "default" | "blocks" | "zebra" | "dimmed-zebra" | "plain"
    )
}

fn first_invalid_color_moved_ws_mode(value: &str) -> Option<&str> {
    value.split(',').find(|mode| {
        !matches!(
            *mode,
            "no" | "ignore-space-change"
                | "ignore-space-at-eol"
                | "ignore-all-space"
                | "allow-indentation-change"
        )
    })
}

fn is_valid_ws_error_highlight(value: &str) -> bool {
    value.split(',').all(|mode| {
        matches!(
            mode,
            "" | "all" | "default" | "old" | "new" | "context" | "none"
        )
    })
}

fn write_stash_show_text(stdout: &mut dyn Write, line_prefix: &str, text: &str) -> io::Result<()> {
    if line_prefix.is_empty() {
        return stdout.write_all(text.as_bytes());
    }
    for line in text.split_inclusive('\n') {
        stdout.write_all(line_prefix.as_bytes())?;
        stdout.write_all(line.as_bytes())?;
    }
    Ok(())
}

fn write_stash_show_blank_line(stdout: &mut dyn Write, line_prefix: &str) -> io::Result<()> {
    if !line_prefix.is_empty() {
        stdout.write_all(line_prefix.as_bytes())?;
    }
    writeln!(stdout)
}

fn stash_show_patch_options(show_args: &StashShowArgs) -> rit_core::PatchRenderOptions {
    rit_core::PatchRenderOptions {
        full_index: show_args.full_index,
        abbrev: show_args.abbrev,
        context_lines: show_args.context_lines,
        inter_hunk_context: show_args.inter_hunk_context,
        default_prefixes: show_args.default_prefixes,
        old_path_prefix: show_args.old_path_prefix.clone(),
        new_path_prefix: show_args.new_path_prefix.clone(),
        new_line_indicator: show_args.new_line_indicator,
        old_line_indicator: show_args.old_line_indicator,
        context_line_indicator: show_args.context_line_indicator,
    }
}

fn stash_show_format(
    repository: &rit_core::Repository,
    explicit_format: Option<StashShowFormat>,
    exit_code: bool,
) -> rit_core::Result<StashShowFormat> {
    if let Some(format) = explicit_format {
        return Ok(format);
    }
    if exit_code {
        return Ok(StashShowFormat::Patch);
    }

    let config_path = repository.common_dir().join("config");
    if !config_path.exists() {
        return Ok(StashShowFormat::Stat);
    }
    let config = rit_core::GitConfig::read(&config_path)?;
    let show_stat = config.get_bool("stash", "showStat", true)?;
    let show_patch = config.get_bool("stash", "showPatch", false)?;
    match (show_stat, show_patch) {
        (false, false) => Ok(StashShowFormat::None),
        (true, false) => Ok(StashShowFormat::Stat),
        (false, true) => Ok(StashShowFormat::Patch),
        (true, true) => Ok(StashShowFormat::StatAndPatch),
    }
}

fn stash_show_exit_code(exit_code: bool, has_changes: bool) -> ExitCode {
    if exit_code && has_changes {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn stash_show_untracked_mode(
    repository: &rit_core::Repository,
    explicit_mode: Option<StashShowUntrackedMode>,
) -> rit_core::Result<StashShowUntrackedMode> {
    if let Some(mode) = explicit_mode {
        return Ok(mode);
    }

    let config_path = repository.common_dir().join("config");
    if !config_path.exists() {
        return Ok(StashShowUntrackedMode::Tracked);
    }
    let config = rit_core::GitConfig::read(&config_path)?;
    if config.get_bool("stash", "showIncludeUntracked", false)? {
        Ok(StashShowUntrackedMode::Include)
    } else {
        Ok(StashShowUntrackedMode::Tracked)
    }
}

struct StashDropArgs {
    index: usize,
    name: String,
    quiet: bool,
}

struct StashPopArgs {
    index: usize,
    name: String,
    quiet: bool,
    restore_index: bool,
}

struct StashApplyArgs {
    index: usize,
    quiet: bool,
    restore_index: bool,
}

struct StashBranchArgs {
    branch: String,
    index: usize,
    name: String,
}

struct StashSaveArgs {
    message: Option<String>,
    quiet: bool,
    keep_index: bool,
    staged: bool,
    include_untracked: bool,
    all: bool,
}

struct StashStoreArgs {
    commit: String,
    message: Option<String>,
}

enum StashExportTarget {
    Print,
    ToRef(String),
}

struct StashExportArgs {
    target: StashExportTarget,
    indices: Vec<usize>,
}

struct StashImportArgs {
    commit: String,
}

#[derive(Default)]
struct StashPushArgs {
    message: Option<String>,
    quiet: bool,
    keep_index: bool,
    staged: bool,
    include_untracked: bool,
    all: bool,
    pathspecs: Vec<String>,
    exit_code: Option<u8>,
}

impl StashPushArgs {
    fn exit(exit_code: u8) -> Self {
        Self {
            exit_code: Some(exit_code),
            ..Self::default()
        }
    }
}

fn parse_stash_push_args(
    args: &[String],
    stderr: &mut dyn Write,
) -> io::Result<Option<StashPushArgs>> {
    let mut parsed = StashPushArgs::default();
    let mut pathspec_file = None;
    let mut pathspec_file_nul = false;
    let after_separator = false;
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if pathspec_from_file_missing_value(args, index, after_separator) {
            write_pathspec_from_file_requires_value(stderr)?;
            return Ok(Some(StashPushArgs::exit(129)));
        }
        match arg.as_str() {
            "--" if !after_separator => {
                parsed.pathspecs.extend(args[index + 1..].iter().cloned());
                break;
            }
            _ if pathspec_args::handle_pathspec_file_option(
                args,
                &mut index,
                after_separator,
                &mut pathspec_file,
                &mut pathspec_file_nul,
            )? => {}
            "-q" | "--quiet" if !after_separator => parsed.quiet = true,
            "-k" | "--keep-index" if !after_separator => parsed.keep_index = true,
            "--no-keep-index" if !after_separator => parsed.keep_index = false,
            "-S" | "--staged" if !after_separator => parsed.staged = true,
            "-u" | "--include-untracked" if !after_separator => parsed.include_untracked = true,
            "-a" | "--all" if !after_separator => parsed.all = true,
            "-m" | "--message" if !after_separator => {
                index += 1;
                let Some(value) = args.get(index) else {
                    writeln!(stderr, "error: switch `m' requires a value")?;
                    return Ok(None);
                };
                parsed.message = Some(value.to_owned());
            }
            _ if arg.starts_with("--message=") && !after_separator => {
                parsed.message = Some(arg.trim_start_matches("--message=").to_owned());
            }
            _ if arg.starts_with('-') && !after_separator => {
                writeln!(stderr, "rit: unsupported stash push option '{arg}'")?;
                return Ok(None);
            }
            _ => {
                parsed.pathspecs.push(arg.to_owned());
            }
        }
        index += 1;
    }

    if pathspec_file_nul && pathspec_file.is_none() {
        write_pathspec_file_nul_requires_file(stderr)?;
        return Ok(Some(StashPushArgs::exit(128)));
    }
    if pathspec_file.is_some() && !parsed.pathspecs.is_empty() {
        write_pathspec_file_cannot_mix_with_args(stderr)?;
        return Ok(Some(StashPushArgs::exit(128)));
    }
    if let Some(file_name) = pathspec_file {
        match pathspec_args::read_pathspecs_from_file(
            &file_name,
            pathspec_file_nul,
            "stash",
            stderr,
        )? {
            pathspec_args::PathspecFileRead::Pathspecs(file_pathspecs) => {
                parsed.pathspecs.extend(file_pathspecs);
            }
            pathspec_args::PathspecFileRead::Error { exit_code } => {
                return Ok(Some(StashPushArgs::exit(exit_code)));
            }
        }
    }

    Ok(Some(parsed))
}

fn parse_stash_save_args(
    args: &[String],
    stderr: &mut dyn Write,
) -> io::Result<Option<StashSaveArgs>> {
    let mut quiet = false;
    let mut keep_index = false;
    let mut staged = false;
    let mut include_untracked = false;
    let mut all = false;
    let mut message_parts = Vec::new();
    for arg in args {
        match arg.as_str() {
            "-q" | "--quiet" if message_parts.is_empty() => quiet = true,
            "-k" | "--keep-index" if message_parts.is_empty() => keep_index = true,
            "--no-keep-index" if message_parts.is_empty() => keep_index = false,
            "-S" | "--staged" if message_parts.is_empty() => staged = true,
            "-u" | "--include-untracked" if message_parts.is_empty() => include_untracked = true,
            "-a" | "--all" if message_parts.is_empty() => all = true,
            "--" if message_parts.is_empty() => {}
            _ if arg.starts_with('-') && message_parts.is_empty() => {
                writeln!(stderr, "rit: unsupported stash save option '{arg}'")?;
                return Ok(None);
            }
            _ => message_parts.push(arg.to_owned()),
        }
    }

    let message = (!message_parts.is_empty()).then(|| message_parts.join(" "));
    Ok(Some(StashSaveArgs {
        message,
        quiet,
        keep_index,
        staged,
        include_untracked,
        all,
    }))
}

fn parse_stash_branch_args(
    args: &[String],
    stderr: &mut dyn Write,
) -> io::Result<Option<StashBranchArgs>> {
    let Some(branch) = args.first() else {
        writeln!(stderr, "rit: stash branch requires a branch name")?;
        return Ok(None);
    };
    if branch.starts_with('-') {
        writeln!(stderr, "rit: stash branch requires a branch name")?;
        return Ok(None);
    }
    if args.len() > 2 {
        writeln!(stderr, "rit: stash branch accepts at most one stash")?;
        return Ok(None);
    }

    let (index, name) =
        parse_stash_name(args.get(1).map(String::as_str).unwrap_or("refs/stash@{0}"))?;
    Ok(Some(StashBranchArgs {
        branch: branch.to_owned(),
        index,
        name,
    }))
}

fn parse_stash_apply_args(
    args: &[String],
    stderr: &mut dyn Write,
) -> io::Result<Option<StashApplyArgs>> {
    let mut quiet = false;
    let mut restore_index = false;
    let mut stash = None;
    for arg in args {
        match arg.as_str() {
            "-q" | "--quiet" => quiet = true,
            "--index" => restore_index = true,
            _ if arg.starts_with('-') => {
                writeln!(stderr, "rit: unsupported stash apply option '{arg}'")?;
                return Ok(None);
            }
            _ if stash.is_none() => stash = Some(arg.as_str()),
            _ => {
                writeln!(stderr, "rit: stash apply accepts at most one stash")?;
                return Ok(None);
            }
        }
    }

    let (index, _) = parse_stash_name(stash.unwrap_or("refs/stash@{0}"))?;
    Ok(Some(StashApplyArgs {
        index,
        quiet,
        restore_index,
    }))
}

fn parse_stash_pop_args(
    args: &[String],
    stderr: &mut dyn Write,
) -> io::Result<Option<StashPopArgs>> {
    let mut quiet = false;
    let mut restore_index = false;
    let mut stash = None;
    for arg in args {
        match arg.as_str() {
            "-q" | "--quiet" => quiet = true,
            "--index" => restore_index = true,
            _ if arg.starts_with('-') => {
                writeln!(stderr, "rit: unsupported stash pop option '{arg}'")?;
                return Ok(None);
            }
            _ if stash.is_none() => stash = Some(arg.as_str()),
            _ => {
                writeln!(stderr, "rit: stash pop accepts at most one stash")?;
                return Ok(None);
            }
        }
    }

    let (index, name) = parse_stash_name(stash.unwrap_or("refs/stash@{0}"))?;
    Ok(Some(StashPopArgs {
        index,
        name,
        quiet,
        restore_index,
    }))
}

fn parse_stash_store_args(
    args: &[String],
    stderr: &mut dyn Write,
) -> io::Result<Option<StashStoreArgs>> {
    let mut message = None;
    let mut commit = None;
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        match arg.as_str() {
            "-q" | "--quiet" => {}
            "-m" | "--message" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    writeln!(stderr, "error: switch `m' requires a value")?;
                    return Ok(None);
                };
                message = Some(value.to_owned());
            }
            _ if arg.starts_with("--message=") => {
                message = Some(arg.trim_start_matches("--message=").to_owned());
            }
            _ if arg.starts_with('-') => {
                writeln!(stderr, "rit: unsupported stash store option '{arg}'")?;
                return Ok(None);
            }
            _ if commit.is_none() => commit = Some(arg.to_owned()),
            _ => {
                writeln!(stderr, "rit: stash store accepts one commit")?;
                return Ok(None);
            }
        }
        index += 1;
    }

    let Some(commit) = commit else {
        writeln!(stderr, "rit: stash store requires a commit")?;
        return Ok(None);
    };
    Ok(Some(StashStoreArgs { commit, message }))
}

fn parse_stash_export_args(
    args: &[String],
    stderr: &mut dyn Write,
) -> io::Result<Option<StashExportArgs>> {
    let mut target = None;
    let mut stashes = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        match arg.as_str() {
            "--print" => {
                if target.is_some() {
                    writeln!(stderr, "rit: stash export accepts one target option")?;
                    return Ok(None);
                }
                target = Some(StashExportTarget::Print);
            }
            "--to-ref" => {
                if target.is_some() {
                    writeln!(stderr, "rit: stash export accepts one target option")?;
                    return Ok(None);
                }
                index += 1;
                let Some(ref_name) = args.get(index) else {
                    writeln!(stderr, "error: option `to-ref' requires a value")?;
                    return Ok(None);
                };
                target = Some(StashExportTarget::ToRef(ref_name.to_owned()));
            }
            _ if arg.starts_with("--to-ref=") => {
                if target.is_some() {
                    writeln!(stderr, "rit: stash export accepts one target option")?;
                    return Ok(None);
                }
                target = Some(StashExportTarget::ToRef(
                    arg.trim_start_matches("--to-ref=").to_owned(),
                ));
            }
            _ if arg.starts_with('-') => {
                writeln!(stderr, "rit: unsupported stash export option '{arg}'")?;
                return Ok(None);
            }
            _ => {
                let (display_index, _) = parse_stash_name(arg)?;
                stashes.push(display_index);
            }
        }
        index += 1;
    }

    let Some(target) = target else {
        writeln!(stderr, "rit: stash export requires --print or --to-ref")?;
        return Ok(None);
    };
    Ok(Some(StashExportArgs {
        target,
        indices: stashes,
    }))
}

fn parse_stash_import_args(
    args: &[String],
    stderr: &mut dyn Write,
) -> io::Result<Option<StashImportArgs>> {
    if args.len() != 1 {
        writeln!(stderr, "rit: stash import requires one commit")?;
        return Ok(None);
    }
    let commit = &args[0];
    if commit.starts_with('-') {
        writeln!(stderr, "rit: stash import requires one commit")?;
        return Ok(None);
    }
    Ok(Some(StashImportArgs {
        commit: commit.to_owned(),
    }))
}

fn parse_stash_drop_args(
    args: &[String],
    stderr: &mut dyn Write,
) -> io::Result<Option<StashDropArgs>> {
    let mut quiet = false;
    let mut stash = None;
    for arg in args {
        match arg.as_str() {
            "-q" | "--quiet" => quiet = true,
            _ if arg.starts_with('-') => {
                writeln!(stderr, "rit: unsupported stash drop option '{arg}'")?;
                return Ok(None);
            }
            _ if stash.is_none() => stash = Some(arg.as_str()),
            _ => {
                writeln!(stderr, "rit: stash drop accepts at most one stash")?;
                return Ok(None);
            }
        }
    }

    let (index, name) = parse_stash_name(stash.unwrap_or("refs/stash@{0}"))?;
    Ok(Some(StashDropArgs { index, name, quiet }))
}

fn parse_stash_name(input: &str) -> io::Result<(usize, String)> {
    if input.chars().all(|character| character.is_ascii_digit()) {
        let index = parse_stash_index(input)?;
        return Ok((index, format!("refs/stash@{{{index}}}")));
    }
    if let Some(index_text) = input
        .strip_prefix("refs/stash@{")
        .and_then(|rest| rest.strip_suffix('}'))
    {
        let index = parse_stash_index(index_text)?;
        return Ok((index, input.to_owned()));
    }
    if let Some(index_text) = input
        .strip_prefix("stash@{")
        .and_then(|rest| rest.strip_suffix('}'))
    {
        let index = parse_stash_index(index_text)?;
        return Ok((index, input.to_owned()));
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("unsupported stash name: {input}"),
    ))
}

fn parse_stash_index(input: &str) -> io::Result<usize> {
    input.parse::<usize>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid stash index: {input}"),
        )
    })
}

fn write_stash_error(stderr: &mut dyn Write, error: rit_core::RitError) -> io::Result<ExitCode> {
    if let rit_core::RitError::InvalidInput { message } = &error {
        if message == "No stash entries found." {
            writeln!(stderr, "{message}")?;
            return Ok(ExitCode::from(1));
        }
        if let Some(count) = message.strip_prefix("log for 'stash' only has ") {
            writeln!(stderr, "fatal: log for 'stash' only has {count}")?;
            return Ok(ExitCode::from(128));
        }
        if message.contains("error: could not restore untracked files from stash") {
            writeln!(stderr, "{message}")?;
            return Ok(ExitCode::from(1));
        }
    }
    write_command_error(stderr, error)
}

fn is_stash_untracked_restore_error(error: &rit_core::RitError) -> bool {
    matches!(
        error,
        rit_core::RitError::InvalidInput { message }
            if message.contains("error: could not restore untracked files from stash")
    )
}

fn write_stash_already_up_to_date_if_no_tracked_changes(
    repository: &rit_core::Repository,
    stdout: &mut dyn Write,
) -> io::Result<()> {
    let status = repository.status_porcelain_v1().map_err(io::Error::other)?;
    let has_tracked_changes = status
        .entries
        .iter()
        .any(|entry| entry.index_status != '?' || entry.worktree_status != '?');
    if !has_tracked_changes {
        writeln!(stdout, "Already up to date.")?;
    }
    Ok(())
}

fn write_stash_human_status(
    repository: &rit_core::Repository,
    stdout: &mut dyn Write,
) -> io::Result<()> {
    let branch_name = repository
        .current_branch_name()
        .map_err(io::Error::other)?
        .unwrap_or_else(|| "HEAD detached".to_owned());
    let status = repository.status_porcelain_v1().map_err(io::Error::other)?;

    writeln!(stdout, "On branch {branch_name}")?;

    let mut wrote_section = false;
    let unstaged = status
        .entries
        .iter()
        .filter(|entry| entry.index_status != '?' && entry.worktree_status != ' ')
        .collect::<Vec<_>>();
    if !unstaged.is_empty() {
        wrote_section = true;
        writeln!(stdout, "Changes not staged for commit:")?;
        writeln!(
            stdout,
            "  (use \"git add <file>...\" to update what will be committed)"
        )?;
        writeln!(
            stdout,
            "  (use \"git restore <file>...\" to discard changes in working directory)"
        )?;
        for entry in unstaged {
            writeln!(
                stdout,
                "\t{:<12}{}",
                human_status_label(entry.worktree_status),
                entry.path
            )?;
        }
    }

    let untracked = status
        .entries
        .iter()
        .filter(|entry| entry.index_status == '?')
        .collect::<Vec<_>>();
    if !untracked.is_empty() {
        if wrote_section {
            writeln!(stdout)?;
        }
        writeln!(stdout, "Untracked files:")?;
        writeln!(
            stdout,
            "  (use \"git add <file>...\" to include in what will be committed)"
        )?;
        for entry in untracked {
            writeln!(stdout, "\t{}", entry.path)?;
        }
    }

    if !status.entries.is_empty() {
        writeln!(stdout)?;
        if status
            .entries
            .iter()
            .all(|entry| entry.index_status == '?' && entry.worktree_status == '?')
        {
            writeln!(
                stdout,
                "nothing added to commit but untracked files present (use \"git add\" to track)"
            )?;
        } else {
            writeln!(
                stdout,
                "no changes added to commit (use \"git add\" and/or \"git commit -a\")"
            )?;
        }
    }
    Ok(())
}

fn human_status_label(status: char) -> &'static str {
    match status {
        'A' => "new file:",
        'D' => "deleted:",
        'M' | 'T' => "modified:",
        _ => "modified:",
    }
}
