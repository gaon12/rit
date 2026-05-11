use crate::{ObjectId, Result, RitError};

use super::{parse_pkt_lines, write_pkt_line};

/// One receive-pack reference update command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReceivePackCommand {
    /// Object ID the server currently has for the ref.
    pub old_id: ObjectId,
    /// Object ID the client wants the ref to point to.
    pub new_id: ObjectId,
    /// Full ref name to update.
    pub name: String,
}

/// Request body for smart HTTP or SSH `git-receive-pack`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReceivePackRequest {
    commands: Vec<ReceivePackCommand>,
    capabilities: Vec<String>,
    pack_data: Vec<u8>,
}

/// Parsed receive-pack status report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReceivePackStatus {
    /// `None` means `unpack ok`; `Some` contains the unpack error message.
    pub unpack_error: Option<String>,
    /// Per-ref command statuses.
    pub commands: Vec<ReceivePackCommandStatus>,
}

/// Per-ref result in a receive-pack status report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReceivePackCommandStatus {
    /// Reference update succeeded.
    Ok {
        /// Updated ref name.
        ref_name: String,
    },
    /// Reference update failed.
    Rejected {
        /// Ref name that failed to update.
        ref_name: String,
        /// Server-provided rejection reason.
        message: String,
    },
}

impl ReceivePackCommand {
    /// Creates a reference update command.
    pub fn new(old_id: ObjectId, new_id: ObjectId, name: impl Into<String>) -> Result<Self> {
        let name = name.into();
        if name.is_empty() {
            return Err(RitError::invalid_input(
                "receive-pack command requires a ref name",
            ));
        }
        Ok(Self {
            old_id,
            new_id,
            name,
        })
    }
}

impl ReceivePackRequest {
    /// Creates a receive-pack request with one or more ref update commands.
    pub fn new(commands: Vec<ReceivePackCommand>) -> Result<Self> {
        if commands.is_empty() {
            return Err(RitError::invalid_input(
                "receive-pack request requires at least one command",
            ));
        }
        Ok(Self {
            commands,
            capabilities: Vec::new(),
            pack_data: Vec::new(),
        })
    }

    /// Adds capabilities sent on the first command line.
    pub fn with_capabilities(mut self, capabilities: Vec<String>) -> Self {
        self.capabilities = capabilities;
        self
    }

    /// Adds raw packfile bytes after the command list flush.
    pub fn with_pack_data(mut self, pack_data: Vec<u8>) -> Self {
        self.pack_data = pack_data;
        self
    }

    /// Serializes the receive-pack request body.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut output = Vec::new();
        for (index, command) in self.commands.iter().enumerate() {
            let mut line = format!("{} {} {}", command.old_id, command.new_id, command.name);
            if index == 0 {
                line.push('\0');
                line.push_str(&self.capabilities.join(" "));
            }
            line.push('\n');
            write_pkt_line(&mut output, line.as_bytes());
        }
        output.extend_from_slice(b"0000");
        output.extend_from_slice(&self.pack_data);
        output
    }
}

impl ReceivePackStatus {
    /// Parses a receive-pack `report-status` response.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let lines = parse_pkt_lines(bytes)?;
        let mut iter = lines.into_iter();
        let Some(unpack_line) = iter.next() else {
            return Err(RitError::invalid_input("empty receive-pack status"));
        };
        let unpack_error = parse_receive_pack_unpack_status(&unpack_line)?;
        let mut commands = Vec::new();
        for line in iter {
            if line.is_empty() {
                break;
            }
            commands.push(parse_receive_pack_command_status(&line)?);
        }
        if commands.is_empty() {
            return Err(RitError::invalid_input(
                "receive-pack status has no command results",
            ));
        }
        Ok(Self {
            unpack_error,
            commands,
        })
    }
}

fn parse_receive_pack_unpack_status(line: &[u8]) -> Result<Option<String>> {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    let line_text = std::str::from_utf8(line)
        .map_err(|_| RitError::invalid_input("receive-pack status is not UTF-8"))?;
    let Some(result) = line_text.strip_prefix("unpack ") else {
        return Err(RitError::invalid_input(
            "receive-pack status is missing unpack result",
        ));
    };
    if result == "ok" {
        Ok(None)
    } else if result.is_empty() {
        Err(RitError::invalid_input(
            "receive-pack unpack result is empty",
        ))
    } else {
        Ok(Some(result.to_owned()))
    }
}

fn parse_receive_pack_command_status(line: &[u8]) -> Result<ReceivePackCommandStatus> {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    let line_text = std::str::from_utf8(line)
        .map_err(|_| RitError::invalid_input("receive-pack command status is not UTF-8"))?;
    if let Some(ref_name) = line_text.strip_prefix("ok ") {
        if ref_name.is_empty() {
            return Err(RitError::invalid_input(
                "receive-pack ok status is missing ref name",
            ));
        }
        return Ok(ReceivePackCommandStatus::Ok {
            ref_name: ref_name.to_owned(),
        });
    }
    if let Some(rest) = line_text.strip_prefix("ng ") {
        let Some((ref_name, message)) = rest.split_once(' ') else {
            return Err(RitError::invalid_input(
                "receive-pack rejection is missing message",
            ));
        };
        if ref_name.is_empty() || message.is_empty() {
            return Err(RitError::invalid_input(
                "receive-pack rejection is missing ref name or message",
            ));
        }
        return Ok(ReceivePackCommandStatus::Rejected {
            ref_name: ref_name.to_owned(),
            message: message.to_owned(),
        });
    }
    Err(RitError::invalid_input(
        "unsupported receive-pack command status",
    ))
}
