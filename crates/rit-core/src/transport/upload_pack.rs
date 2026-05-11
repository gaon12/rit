use crate::{ObjectId, Result, RitError};

use super::{read_pkt_line_at, write_pkt_line};

/// Request body for smart HTTP `git-upload-pack`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UploadPackRequest {
    wants: Vec<ObjectId>,
    haves: Vec<ObjectId>,
    capabilities: Vec<String>,
    done: bool,
}

/// ACK status words used by upload-pack negotiation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UploadPackAckStatus {
    /// The object is common, and negotiation should continue.
    Continue,
    /// The object is a common commit.
    Common,
    /// The server is ready to send pack data.
    Ready,
}

/// One upload-pack negotiation response line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UploadPackAcknowledgement {
    /// The server has no common object to acknowledge for this round.
    Nak,
    /// The server acknowledges a common object.
    Ack {
        /// Common object ID reported by the server.
        object_id: ObjectId,
        /// Optional multi_ack status word.
        status: Option<UploadPackAckStatus>,
    },
    /// Server-side protocol error returned as an `ERR` pkt-line.
    Error {
        /// Human-readable error message without the leading `ERR ` marker.
        message: String,
    },
}

/// Parsed upload-pack response up to the start of pack data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UploadPackResponse {
    /// ACK, NAK, or ERR records returned before pack bytes.
    pub acknowledgements: Vec<UploadPackAcknowledgement>,
    /// Raw non-sideband pack bytes when the response switches to `PACK...`.
    pub pack_data: Option<Vec<u8>>,
    /// Multiplexed side-band records when side-band capability was used.
    pub side_bands: Vec<UploadPackSideBand>,
}

/// One side-band record returned by upload-pack.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UploadPackSideBand {
    /// Band 1 contains packfile bytes.
    PackData(Vec<u8>),
    /// Band 2 contains progress output.
    Progress(Vec<u8>),
    /// Band 3 contains server error output.
    Error(Vec<u8>),
}

impl UploadPackRequest {
    /// Creates a request with one or more wanted objects.
    pub fn new(wants: Vec<ObjectId>) -> Result<Self> {
        if wants.is_empty() {
            return Err(RitError::invalid_input(
                "upload-pack request requires at least one want",
            ));
        }
        Ok(Self {
            wants,
            haves: Vec::new(),
            capabilities: Vec::new(),
            done: true,
        })
    }

    /// Adds capabilities sent on the first `want` line.
    pub fn with_capabilities(mut self, capabilities: Vec<String>) -> Self {
        self.capabilities = capabilities;
        self
    }

    /// Adds existing local objects as `have` lines.
    pub fn with_haves(mut self, haves: Vec<ObjectId>) -> Self {
        self.haves = haves;
        self
    }

    /// Serializes the request as pkt-lines.
    pub fn to_pkt_lines(&self) -> Vec<u8> {
        let mut output = Vec::new();
        for (index, object_id) in self.wants.iter().enumerate() {
            let mut line = format!("want {object_id}");
            if index == 0 && !self.capabilities.is_empty() {
                line.push(' ');
                line.push_str(&self.capabilities.join(" "));
            }
            line.push('\n');
            write_pkt_line(&mut output, line.as_bytes());
        }
        for object_id in &self.haves {
            write_pkt_line(&mut output, format!("have {object_id}\n").as_bytes());
        }
        if self.done {
            write_pkt_line(&mut output, b"done\n");
        } else {
            output.extend_from_slice(b"0000");
        }
        output
    }
}

impl UploadPackResponse {
    /// Parses upload-pack ACK/NAK pkt-lines and raw `PACK` data.
    ///
    /// Side-band pack data is still handled by future transport work; this
    /// parser intentionally rejects unknown pkt-line payloads so callers do not
    /// accidentally treat multiplexed progress or errors as pack data.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let mut acknowledgements = Vec::new();
        let mut pack_data = None;
        let mut side_bands = Vec::new();
        let mut position = 0;

        while position < bytes.len() {
            if bytes[position..].starts_with(b"PACK") {
                pack_data = Some(bytes[position..].to_vec());
                break;
            }

            let (payload, next_position) = read_pkt_line_at(bytes, position)?;
            position = next_position;
            if payload.is_empty() {
                continue;
            }
            if let Some(side_band) = parse_upload_pack_side_band(&payload) {
                side_bands.push(side_band);
            } else {
                acknowledgements.push(parse_upload_pack_acknowledgement(&payload)?);
            }
        }

        Ok(Self {
            acknowledgements,
            pack_data,
            side_bands,
        })
    }

    /// Returns raw pack bytes from either non-sideband or side-band data.
    pub fn pack_bytes(&self) -> Result<Option<Vec<u8>>> {
        if let Some(pack_data) = &self.pack_data {
            return Ok(Some(pack_data.clone()));
        }

        let mut pack = Vec::new();
        let mut saw_pack_side_band = false;
        for side_band in &self.side_bands {
            match side_band {
                UploadPackSideBand::PackData(data) => {
                    saw_pack_side_band = true;
                    pack.extend_from_slice(data);
                }
                UploadPackSideBand::Progress(_) => {}
                UploadPackSideBand::Error(data) => {
                    let message = String::from_utf8_lossy(data).trim().to_owned();
                    return Err(RitError::invalid_input(format!(
                        "upload-pack side-band error: {message}"
                    )));
                }
            }
        }

        Ok(saw_pack_side_band.then_some(pack))
    }
}

fn parse_upload_pack_acknowledgement(line: &[u8]) -> Result<UploadPackAcknowledgement> {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    let line_text = std::str::from_utf8(line)
        .map_err(|_| RitError::invalid_input("upload-pack response is not UTF-8"))?;
    if line_text == "NAK" {
        return Ok(UploadPackAcknowledgement::Nak);
    }
    if let Some(message) = line_text.strip_prefix("ERR ") {
        return Ok(UploadPackAcknowledgement::Error {
            message: message.to_owned(),
        });
    }

    let parts = line_text.split_whitespace().collect::<Vec<_>>();
    match parts.as_slice() {
        ["ACK", object_id] => Ok(UploadPackAcknowledgement::Ack {
            object_id: ObjectId::from_hex(object_id)?,
            status: None,
        }),
        ["ACK", object_id, status] => Ok(UploadPackAcknowledgement::Ack {
            object_id: ObjectId::from_hex(object_id)?,
            status: Some(parse_upload_pack_ack_status(status)?),
        }),
        _ => Err(RitError::invalid_input(
            "unsupported upload-pack response line",
        )),
    }
}

fn parse_upload_pack_ack_status(status: &str) -> Result<UploadPackAckStatus> {
    match status {
        "continue" => Ok(UploadPackAckStatus::Continue),
        "common" => Ok(UploadPackAckStatus::Common),
        "ready" => Ok(UploadPackAckStatus::Ready),
        _ => Err(RitError::invalid_input(format!(
            "unsupported upload-pack ACK status: {status}"
        ))),
    }
}

fn parse_upload_pack_side_band(payload: &[u8]) -> Option<UploadPackSideBand> {
    let (&band, data) = payload.split_first()?;
    match band {
        1 => Some(UploadPackSideBand::PackData(data.to_vec())),
        2 => Some(UploadPackSideBand::Progress(data.to_vec())),
        3 => Some(UploadPackSideBand::Error(data.to_vec())),
        _ => None,
    }
}
