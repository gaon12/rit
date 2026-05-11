use std::path::PathBuf;

use crate::{ObjectId, Result, RitError};

/// One simple fetch refspec, such as `refs/heads/main:refs/remotes/origin/main`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FetchRefSpec {
    /// Allow non-fast-forward updates. The first implementation records this
    /// bit but does not yet perform ancestry checks.
    pub force: bool,
    /// Source ref or revision in the remote repository.
    pub source: String,
    /// Destination ref to update in the local repository.
    pub destination: String,
}

/// Git smart HTTP service endpoints.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SmartHttpService {
    /// Fetch/clone service.
    UploadPack,
    /// Push service.
    ReceivePack,
}

impl SmartHttpService {
    /// Returns the service name used in Git HTTP query parameters and MIME
    /// types.
    pub fn name(self) -> &'static str {
        match self {
            Self::UploadPack => "git-upload-pack",
            Self::ReceivePack => "git-receive-pack",
        }
    }
}

/// Request metadata for Git smart HTTP reference discovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmartHttpRequest {
    /// URL for `GET` reference discovery.
    pub info_refs_url: String,
    /// Expected advertisement content type.
    pub advertisement_content_type: String,
}

/// One reference advertised by a smart HTTP service.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdvertisedRef {
    /// Object ID currently stored in the advertised ref.
    pub object_id: ObjectId,
    /// Full ref name, such as `refs/heads/main`.
    pub name: String,
}

/// Parsed smart HTTP reference advertisement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmartHttpAdvertisement {
    /// Service that produced the advertisement.
    pub service: SmartHttpService,
    /// Capabilities listed on the first ref record.
    pub capabilities: Vec<String>,
    /// Advertised refs in stream order.
    pub refs: Vec<AdvertisedRef>,
}

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
            if let Some(side_band) = parse_upload_pack_side_band(&payload)? {
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
}

impl SmartHttpAdvertisement {
    /// Parses the pkt-line response body returned by smart HTTP `info/refs`.
    pub fn parse(service: SmartHttpService, bytes: &[u8]) -> Result<Self> {
        let lines = parse_pkt_lines(bytes)?;
        let mut iter = lines.into_iter();
        let Some(service_line) = iter.next() else {
            return Err(RitError::invalid_input("empty smart HTTP advertisement"));
        };
        let expected = format!("# service={}\n", service.name());
        if service_line != expected.as_bytes() {
            return Err(RitError::invalid_input(
                "smart HTTP service header mismatch",
            ));
        }
        match iter.next() {
            Some(line) if line.is_empty() => {}
            _ => {
                return Err(RitError::invalid_input(
                    "smart HTTP advertisement missing service flush",
                ));
            }
        }

        let mut capabilities = Vec::new();
        let mut refs = Vec::new();
        for line in iter {
            if line.is_empty() {
                break;
            }
            let (advertised_ref, advertised_capabilities) =
                parse_advertised_ref_line(&line, refs.is_empty())?;
            if let Some(advertised_capabilities) = advertised_capabilities {
                capabilities = advertised_capabilities;
            }
            refs.push(advertised_ref);
        }

        Ok(Self {
            service,
            capabilities,
            refs,
        })
    }
}

impl FetchRefSpec {
    /// Parses one fetch refspec.
    pub fn parse(input: &str) -> Result<Self> {
        let (force, body) = if let Some(rest) = input.strip_prefix('+') {
            (true, rest)
        } else {
            (false, input)
        };
        let Some((source, destination)) = body.split_once(':') else {
            return Err(RitError::invalid_input(format!(
                "unsupported fetch refspec: {input}"
            )));
        };
        if source.is_empty() || destination.is_empty() {
            return Err(RitError::invalid_input(format!(
                "invalid fetch refspec: {input}"
            )));
        }
        Ok(Self {
            force,
            source: source.to_owned(),
            destination: destination.to_owned(),
        })
    }
}

fn parse_pkt_lines(bytes: &[u8]) -> Result<Vec<Vec<u8>>> {
    let mut lines = Vec::new();
    let mut position = 0;
    while position < bytes.len() {
        let (payload, next_position) = read_pkt_line_at(bytes, position)?;
        lines.push(payload);
        position = next_position;
    }
    Ok(lines)
}

fn read_pkt_line_at(bytes: &[u8], position: usize) -> Result<(Vec<u8>, usize)> {
    let length_end = position + 4;
    let Some(length_bytes) = bytes.get(position..length_end) else {
        return Err(RitError::invalid_input("truncated pkt-line length"));
    };
    let length_text = std::str::from_utf8(length_bytes)
        .map_err(|_| RitError::invalid_input("pkt-line length is not UTF-8"))?;
    let length = u16::from_str_radix(length_text, 16)
        .map_err(|_| RitError::invalid_input("invalid pkt-line length"))? as usize;

    if length == 0 {
        return Ok((Vec::new(), length_end));
    }
    if length < 4 {
        return Err(RitError::invalid_input(
            "pkt-line length is smaller than header",
        ));
    }
    let payload_start = length_end;
    let payload_len = length - 4;
    let payload_end = payload_start + payload_len;
    let Some(payload) = bytes.get(payload_start..payload_end) else {
        return Err(RitError::invalid_input("truncated pkt-line payload"));
    };
    Ok((payload.to_vec(), payload_end))
}

fn write_pkt_line(output: &mut Vec<u8>, payload: &[u8]) {
    let length = payload.len() + 4;
    output.extend_from_slice(format!("{length:04x}").as_bytes());
    output.extend_from_slice(payload);
}

fn parse_advertised_ref_line(
    line: &[u8],
    first_ref: bool,
) -> Result<(AdvertisedRef, Option<Vec<String>>)> {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    let line_text = std::str::from_utf8(line)
        .map_err(|_| RitError::invalid_input("advertised ref is not UTF-8"))?;
    let (record, capabilities) = if first_ref {
        match line_text.split_once('\0') {
            Some((record, capabilities)) => (
                record,
                Some(
                    capabilities
                        .split_whitespace()
                        .map(ToOwned::to_owned)
                        .collect::<Vec<_>>(),
                ),
            ),
            None => (line_text, Some(Vec::new())),
        }
    } else {
        (line_text, None)
    };
    let Some((object_id, name)) = record.split_once(' ') else {
        return Err(RitError::invalid_input("malformed advertised ref"));
    };
    if name.is_empty() {
        return Err(RitError::invalid_input("advertised ref has empty name"));
    }
    Ok((
        AdvertisedRef {
            object_id: ObjectId::from_hex(object_id)?,
            name: name.to_owned(),
        },
        capabilities,
    ))
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

fn parse_upload_pack_side_band(payload: &[u8]) -> Result<Option<UploadPackSideBand>> {
    let Some((&band, data)) = payload.split_first() else {
        return Ok(None);
    };
    match band {
        1 => Ok(Some(UploadPackSideBand::PackData(data.to_vec()))),
        2 => Ok(Some(UploadPackSideBand::Progress(data.to_vec()))),
        3 => Ok(Some(UploadPackSideBand::Error(data.to_vec()))),
        _ => Ok(None),
    }
}

/// Transport protocol family inferred from a repository location.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransportProtocol {
    /// Local filesystem path.
    Local,
    /// Plain HTTP remote.
    Http,
    /// HTTPS remote.
    Https,
    /// SSH remote, including scp-like `host:path` forms.
    Ssh,
}

/// Parsed repository location used to route transport implementations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransportLocation {
    original: String,
    protocol: TransportProtocol,
}

impl TransportLocation {
    /// Classifies a Git repository argument as local, HTTP(S), or SSH.
    pub fn parse(input: &str) -> Self {
        let protocol = if input.starts_with("http://") {
            TransportProtocol::Http
        } else if input.starts_with("https://") {
            TransportProtocol::Https
        } else if input.starts_with("ssh://") || looks_like_scp_location(input) {
            TransportProtocol::Ssh
        } else {
            TransportProtocol::Local
        };
        Self {
            original: input.to_owned(),
            protocol,
        }
    }

    /// Returns the original repository argument.
    pub fn original(&self) -> &str {
        &self.original
    }

    /// Returns the classified transport protocol.
    pub fn protocol(&self) -> TransportProtocol {
        self.protocol
    }

    /// Returns true when this location can be opened directly from the
    /// filesystem.
    pub fn is_local(&self) -> bool {
        self.protocol == TransportProtocol::Local
    }

    /// Returns the local filesystem path for local locations.
    pub fn local_path(&self) -> Option<PathBuf> {
        self.is_local().then(|| PathBuf::from(&self.original))
    }

    /// Builds the smart HTTP `info/refs` discovery request for HTTP(S)
    /// locations.
    pub fn smart_http_info_refs(&self, service: SmartHttpService) -> Result<SmartHttpRequest> {
        match self.protocol {
            TransportProtocol::Http | TransportProtocol::Https => {
                let base = self.original.trim_end_matches('/');
                let service_name = service.name();
                Ok(SmartHttpRequest {
                    info_refs_url: format!("{base}/info/refs?service={service_name}"),
                    advertisement_content_type: format!(
                        "application/x-{service_name}-advertisement"
                    ),
                })
            }
            _ => Err(RitError::invalid_input(format!(
                "smart HTTP requires an HTTP(S) location: {}",
                self.original
            ))),
        }
    }
}

fn looks_like_scp_location(input: &str) -> bool {
    if is_windows_drive_path(input) {
        return false;
    }
    let Some(colon_index) = input.find(':') else {
        return false;
    };
    let before_colon = &input[..colon_index];
    !before_colon.is_empty()
        && !before_colon.contains('/')
        && !before_colon.contains('\\')
        && input[colon_index + 1..].contains('/')
}

fn is_windows_drive_path(input: &str) -> bool {
    let bytes = input.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

#[cfg(test)]
mod tests {
    use super::{
        FetchRefSpec, SmartHttpAdvertisement, SmartHttpService, TransportLocation,
        TransportProtocol, UploadPackAckStatus, UploadPackAcknowledgement, UploadPackResponse,
        UploadPackSideBand,
    };
    use crate::ObjectId;

    #[test]
    fn classifies_local_paths() {
        assert_eq!(
            TransportLocation::parse("../repo").protocol(),
            TransportProtocol::Local
        );
        assert_eq!(
            TransportLocation::parse("C:/repo").protocol(),
            TransportProtocol::Local
        );
        assert_eq!(
            TransportLocation::parse("C:\\repo").protocol(),
            TransportProtocol::Local
        );
    }

    #[test]
    fn classifies_http_and_ssh_locations() {
        assert_eq!(
            TransportLocation::parse("http://example.test/repo.git").protocol(),
            TransportProtocol::Http
        );
        assert_eq!(
            TransportLocation::parse("https://example.test/repo.git").protocol(),
            TransportProtocol::Https
        );
        assert_eq!(
            TransportLocation::parse("ssh://example.test/repo.git").protocol(),
            TransportProtocol::Ssh
        );
        assert_eq!(
            TransportLocation::parse("git@example.test:org/repo.git").protocol(),
            TransportProtocol::Ssh
        );
    }

    #[test]
    fn parses_simple_fetch_refspecs() {
        let refspec =
            FetchRefSpec::parse("+refs/heads/main:refs/remotes/origin/main").expect("valid");
        assert!(refspec.force);
        assert_eq!(refspec.source, "refs/heads/main");
        assert_eq!(refspec.destination, "refs/remotes/origin/main");

        assert!(FetchRefSpec::parse("refs/heads/main").is_err());
        assert!(FetchRefSpec::parse(":refs/heads/main").is_err());
    }

    #[test]
    fn builds_smart_http_discovery_requests() {
        let location = TransportLocation::parse("https://example.test/repo.git/");
        let request = location
            .smart_http_info_refs(SmartHttpService::UploadPack)
            .expect("https supports smart http");
        assert_eq!(
            request.info_refs_url,
            "https://example.test/repo.git/info/refs?service=git-upload-pack"
        );
        assert_eq!(
            request.advertisement_content_type,
            "application/x-git-upload-pack-advertisement"
        );

        assert!(
            TransportLocation::parse("../repo")
                .smart_http_info_refs(SmartHttpService::UploadPack)
                .is_err()
        );
    }

    #[test]
    fn parses_smart_http_advertisements() {
        let body = concat!(
            "001e# service=git-upload-pack\n",
            "0000",
            "005295dcfa3633004da0049d3d0fa03f80589cbcaf31 refs/heads/maint\0multi_ack thin-pack\n",
            "003fd049f6c27a2244e12041955e262a404c7faba355 refs/heads/master\n",
            "0000"
        );

        let advertisement =
            SmartHttpAdvertisement::parse(SmartHttpService::UploadPack, body.as_bytes())
                .expect("advertisement should parse");

        assert_eq!(advertisement.service, SmartHttpService::UploadPack);
        assert_eq!(advertisement.capabilities, ["multi_ack", "thin-pack"]);
        assert_eq!(advertisement.refs.len(), 2);
        assert_eq!(advertisement.refs[0].name, "refs/heads/maint");
        assert_eq!(advertisement.refs[1].name, "refs/heads/master");
    }

    #[test]
    fn rejects_wrong_smart_http_service_header() {
        let body = "001e# service=git-upload-pack\n0000";
        let error = SmartHttpAdvertisement::parse(SmartHttpService::ReceivePack, body.as_bytes())
            .expect_err("service should be checked");

        assert!(error.to_string().contains("service header"));
    }

    #[test]
    fn builds_upload_pack_request_pkt_lines() {
        let want = ObjectId::from_hex("0a53e9ddeaddad63ad106860237bbf53411d11a7").expect("want");
        let have = ObjectId::from_hex("441b40d833fdfa93eb2908e52742248faf0ee993").expect("have");
        let request = super::UploadPackRequest::new(vec![want])
            .expect("request should build")
            .with_capabilities(vec!["multi_ack".to_owned(), "thin-pack".to_owned()])
            .with_haves(vec![have]);

        let body = String::from_utf8(request.to_pkt_lines()).expect("pkt-lines are UTF-8");

        assert_eq!(
            body,
            concat!(
                "0046want 0a53e9ddeaddad63ad106860237bbf53411d11a7 multi_ack thin-pack\n",
                "0032have 441b40d833fdfa93eb2908e52742248faf0ee993\n",
                "0009done\n"
            )
        );
    }

    #[test]
    fn rejects_upload_pack_request_without_wants() {
        let error = super::UploadPackRequest::new(Vec::new()).expect_err("want is required");

        assert!(error.to_string().contains("at least one want"));
    }

    #[test]
    fn parses_upload_pack_nak_and_raw_pack_response() {
        let response =
            UploadPackResponse::parse(b"0008NAK\nPACK\x00\x00\x00\x02payload").expect("response");

        assert_eq!(response.acknowledgements, [UploadPackAcknowledgement::Nak]);
        assert_eq!(
            response.pack_data,
            Some(b"PACK\x00\x00\x00\x02payload".to_vec())
        );
        assert!(response.side_bands.is_empty());
    }

    #[test]
    fn parses_upload_pack_ack_statuses() {
        let first = "7e47fe2bd8d01d481f44d7af0531bd93d3b21c01";
        let second = "74730d410fcb6603ace96f1dc55ea6196122532d";
        let response = UploadPackResponse::parse(
            format!("003aACK {first} continue\n0037ACK {second} ready\n").as_bytes(),
        )
        .expect("response");

        assert_eq!(
            response.acknowledgements,
            [
                UploadPackAcknowledgement::Ack {
                    object_id: ObjectId::from_hex(first).expect("first object"),
                    status: Some(UploadPackAckStatus::Continue),
                },
                UploadPackAcknowledgement::Ack {
                    object_id: ObjectId::from_hex(second).expect("second object"),
                    status: Some(UploadPackAckStatus::Ready),
                },
            ]
        );
        assert_eq!(response.pack_data, None);
        assert!(response.side_bands.is_empty());
    }

    #[test]
    fn parses_upload_pack_error_response() {
        let response = UploadPackResponse::parse(b"0014ERR unknown ref\n").expect("response");

        assert_eq!(
            response.acknowledgements,
            [UploadPackAcknowledgement::Error {
                message: "unknown ref".to_owned(),
            }]
        );
        assert!(response.side_bands.is_empty());
    }

    #[test]
    fn parses_upload_pack_side_band_packets() {
        let response = UploadPackResponse::parse(
            b"0008NAK\n000d\x01PACKdata000e\x02Counting\n000b\x03fatal\n0000",
        )
        .expect("response");

        assert_eq!(response.acknowledgements, [UploadPackAcknowledgement::Nak]);
        assert_eq!(
            response.side_bands,
            [
                UploadPackSideBand::PackData(b"PACKdata".to_vec()),
                UploadPackSideBand::Progress(b"Counting\n".to_vec()),
                UploadPackSideBand::Error(b"fatal\n".to_vec()),
            ]
        );
        assert_eq!(response.pack_data, None);
    }

    #[test]
    fn rejects_unknown_upload_pack_response_lines() {
        let error =
            UploadPackResponse::parse(b"000dprogress\n").expect_err("line should be rejected");

        assert!(error.to_string().contains("unsupported upload-pack"));
    }
}
