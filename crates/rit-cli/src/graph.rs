use crate::op::json_escape;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

pub fn graph_command(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let Some(args) = parse_graph_args(args, stderr)? else {
        return Ok(ExitCode::from(129));
    };
    let repository = match rit_core::Repository::discover(".") {
        Ok(repository) => repository,
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            return Ok(ExitCode::from(128));
        }
    };
    let graph = match repository.local_graph() {
        Ok(graph) => graph,
        Err(error) => {
            writeln!(stderr, "rit: {error}")?;
            return Ok(ExitCode::from(1));
        }
    };
    if args.json {
        write_graph_json(stdout, &graph)?;
    } else {
        write_graph_text(stdout, &graph)?;
    }
    Ok(ExitCode::SUCCESS)
}

struct GraphArgs {
    json: bool,
}

fn parse_graph_args(args: &[String], stderr: &mut dyn Write) -> io::Result<Option<GraphArgs>> {
    let mut parsed = GraphArgs { json: false };
    for arg in args {
        match arg.as_str() {
            "--json" => parsed.json = true,
            unsupported => {
                writeln!(stderr, "rit: unsupported graph option '{unsupported}'")?;
                return Ok(None);
            }
        }
    }
    Ok(Some(parsed))
}

fn write_graph_text(stdout: &mut dyn Write, graph: &rit_core::LocalGraph) -> io::Result<()> {
    writeln!(
        stdout,
        "HEAD: {} {}",
        graph.head.branch.as_deref().unwrap_or("(detached)"),
        graph
            .head
            .target
            .map(|target| target.to_hex())
            .unwrap_or_else(|| "(unborn)".to_owned())
    )?;
    writeln!(stdout, "Branches:")?;
    if graph.branches.is_empty() {
        writeln!(stdout, "  (none)")?;
    }
    for branch in &graph.branches {
        let marker = if branch.current { "*" } else { " " };
        write!(stdout, "{marker} {} {}", branch.name, branch.target)?;
        if let Some(upstream) = &branch.upstream {
            write!(
                stdout,
                " upstream={} ahead={} behind={}",
                upstream.name, branch.ahead, branch.behind
            )?;
        }
        write!(stdout, " status={}", branch_status(branch))?;
        writeln!(stdout)?;
    }
    writeln!(stdout, "Stashes:")?;
    if graph.stashes.is_empty() {
        writeln!(stdout, "  (none)")?;
    }
    for stash in &graph.stashes {
        writeln!(stdout, "  stash@{{{}}}: {}", stash.index, stash.message)?;
    }
    writeln!(stdout, "Worktrees:")?;
    if graph.worktrees.is_empty() {
        writeln!(stdout, "  (none)")?;
    }
    for worktree in &graph.worktrees {
        let marker = if worktree.current { "*" } else { " " };
        writeln!(
            stdout,
            "{marker} {} {}",
            worktree.id,
            path_text(worktree.path.as_deref())
        )?;
    }
    Ok(())
}

fn branch_status(branch: &rit_core::LocalGraphBranch) -> &'static str {
    if branch.diverged {
        "diverged"
    } else if branch.unpushed {
        "unpushed"
    } else if branch.behind > 0 {
        "behind"
    } else if branch.upstream.is_some() {
        "synced"
    } else {
        "local"
    }
}

fn path_text(path: Option<&Path>) -> String {
    path.map(|path| path.display().to_string())
        .unwrap_or_else(|| "(unknown)".to_owned())
}

fn write_graph_json(stdout: &mut dyn Write, graph: &rit_core::LocalGraph) -> io::Result<()> {
    writeln!(stdout, "{{")?;
    writeln!(stdout, "  \"head\": {{")?;
    writeln!(
        stdout,
        "    \"branch\": {},",
        json_optional_string(graph.head.branch.as_deref())
    )?;
    writeln!(
        stdout,
        "    \"target\": {}",
        json_optional_string(graph.head.target.map(|target| target.to_hex()).as_deref())
    )?;
    writeln!(stdout, "  }},")?;
    writeln!(stdout, "  \"branches\": [")?;
    for (index, branch) in graph.branches.iter().enumerate() {
        if index > 0 {
            writeln!(stdout, ",")?;
        }
        write_branch_json(stdout, branch)?;
    }
    writeln!(stdout)?;
    writeln!(stdout, "  ],")?;
    writeln!(stdout, "  \"stashes\": [")?;
    for (index, stash) in graph.stashes.iter().enumerate() {
        if index > 0 {
            writeln!(stdout, ",")?;
        }
        write!(
            stdout,
            "    {{\"index\": {}, \"message\": \"{}\"}}",
            stash.index,
            json_escape(&stash.message)
        )?;
    }
    writeln!(stdout)?;
    writeln!(stdout, "  ],")?;
    writeln!(stdout, "  \"worktrees\": [")?;
    for (index, worktree) in graph.worktrees.iter().enumerate() {
        if index > 0 {
            writeln!(stdout, ",")?;
        }
        write!(
            stdout,
            "    {{\"id\": \"{}\", \"path\": {}, \"current\": {}}}",
            json_escape(&worktree.id),
            json_optional_string(
                worktree
                    .path
                    .as_deref()
                    .map(|path| path.to_string_lossy())
                    .as_deref()
            ),
            worktree.current
        )?;
    }
    writeln!(stdout)?;
    writeln!(stdout, "  ]")?;
    writeln!(stdout, "}}")
}

fn write_branch_json(
    stdout: &mut dyn Write,
    branch: &rit_core::LocalGraphBranch,
) -> io::Result<()> {
    writeln!(stdout, "    {{")?;
    writeln!(stdout, "      \"name\": \"{}\",", json_escape(&branch.name))?;
    writeln!(stdout, "      \"target\": \"{}\",", branch.target)?;
    writeln!(stdout, "      \"current\": {},", branch.current)?;
    if let Some(upstream) = &branch.upstream {
        writeln!(stdout, "      \"upstream\": {{")?;
        writeln!(
            stdout,
            "        \"name\": \"{}\",",
            json_escape(&upstream.name)
        )?;
        writeln!(stdout, "        \"target\": \"{}\"", upstream.target)?;
        writeln!(stdout, "      }},")?;
    } else {
        writeln!(stdout, "      \"upstream\": null,")?;
    }
    writeln!(stdout, "      \"ahead\": {},", branch.ahead)?;
    writeln!(stdout, "      \"behind\": {},", branch.behind)?;
    writeln!(stdout, "      \"unpushed\": {},", branch.unpushed)?;
    writeln!(stdout, "      \"diverged\": {}", branch.diverged)?;
    write!(stdout, "    }}")
}

fn json_optional_string(value: Option<&str>) -> String {
    value
        .map(|value| format!("\"{}\"", json_escape(value)))
        .unwrap_or_else(|| "null".to_owned())
}

#[cfg(test)]
mod tests {
    use super::{write_graph_json, write_graph_text};
    use rit_core::{
        LocalGraph, LocalGraphBranch, LocalGraphHead, LocalGraphStash, LocalGraphUpstream,
        LocalGraphWorktree, ObjectId,
    };
    use std::path::PathBuf;

    #[test]
    fn graph_text_prints_branch_relationships() {
        let graph = sample_graph();
        let mut output = Vec::new();

        write_graph_text(&mut output, &graph).expect("graph should write");
        let output = String::from_utf8(output).expect("output should be utf-8");

        assert!(output.contains("HEAD: master"));
        assert!(output.contains("* master"));
        assert!(output.contains("upstream=refs/remotes/origin/main ahead=1 behind=1"));
        assert!(output.contains("status=diverged"));
        assert!(output.contains("stash@{0}: WIP"));
        assert!(output.contains("* main"));
    }

    #[test]
    fn graph_json_prints_typed_fields() {
        let graph = sample_graph();
        let mut output = Vec::new();

        write_graph_json(&mut output, &graph).expect("graph should write");
        let output = String::from_utf8(output).expect("output should be utf-8");

        assert!(output.contains("\"branch\": \"master\""));
        assert!(output.contains("\"name\": \"master\""));
        assert!(output.contains("\"upstream\": {"));
        assert!(output.contains("\"ahead\": 1"));
        assert!(output.contains("\"diverged\": true"));
        assert!(output.contains("\"worktrees\""));
    }

    fn sample_graph() -> LocalGraph {
        let local = object_id("1111111111111111111111111111111111111111");
        let upstream = object_id("2222222222222222222222222222222222222222");
        LocalGraph {
            head: LocalGraphHead {
                branch: Some("master".to_owned()),
                target: Some(local),
            },
            branches: vec![LocalGraphBranch {
                name: "master".to_owned(),
                target: local,
                current: true,
                upstream: Some(LocalGraphUpstream {
                    name: "refs/remotes/origin/main".to_owned(),
                    target: upstream,
                }),
                ahead: 1,
                behind: 1,
                unpushed: true,
                diverged: true,
            }],
            stashes: vec![LocalGraphStash {
                index: 0,
                message: "WIP".to_owned(),
            }],
            worktrees: vec![LocalGraphWorktree {
                id: "main".to_owned(),
                path: Some(PathBuf::from("C:/repo")),
                current: true,
            }],
        }
    }

    fn object_id(hex: &str) -> ObjectId {
        ObjectId::from_hex(hex).expect("object id should parse")
    }
}
