use std::{
    io::{Read, Write},
    net::TcpStream,
    path::PathBuf,
    time::Duration,
};

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

/// Request metadata for a smart HTTP service POST.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmartHttpPostRequest {
    /// URL for the service request.
    pub url: String,
    /// Request content type.
    pub content_type: String,
    /// Expected response content type.
    pub response_content_type: String,
    /// Serialized pkt-line body.
    pub body: Vec<u8>,
}

/// Minimal HTTP response returned by the smart HTTP client.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmartHttpResponse {
    /// Numeric HTTP status code.
    pub status_code: u16,
    /// Response headers in received order.
    pub headers: Vec<(String, String)>,
    /// Raw response body.
    pub body: Vec<u8>,
}

/// Command metadata needed to start a Git service over SSH.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SshServiceCommand {
    /// Git service to request on the remote side.
    pub service: SmartHttpService,
    /// Optional SSH username from `user@host` locations.
    pub user: Option<String>,
    /// SSH host name.
    pub host: String,
    /// Repository path passed to the remote Git service.
    pub path: String,
    /// Remote shell command, such as `git-upload-pack 'repo.git'`.
    pub remote_command: String,
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

impl SmartHttpResponse {
    /// Returns a header value with case-insensitive name matching.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(header_name, _)| header_name.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
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

/// Small blocking HTTP client for the first smart HTTP implementation.
#[derive(Clone, Debug)]
pub struct BlockingSmartHttpClient {
    timeout: Duration,
}

impl Default for BlockingSmartHttpClient {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
        }
    }
}

impl BlockingSmartHttpClient {
    /// Builds a client with a custom connect/read/write timeout.
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }

    /// Performs smart HTTP reference discovery with a GET request.
    pub fn get_info_refs(
        &self,
        location: &TransportLocation,
        service: SmartHttpService,
    ) -> Result<SmartHttpResponse> {
        let request = location.smart_http_info_refs(service)?;
        let response = self.send_http_request("GET", &request.info_refs_url, None, &[])?;
        validate_smart_http_response(
            &response,
            &request.advertisement_content_type,
            &[200, 304],
            SmartHttpBodyCheck::InfoRefsAdvertisement,
        )?;
        Ok(response)
    }

    /// Discovers and parses refs advertised by a smart HTTP service.
    pub fn discover_refs(
        &self,
        location: &TransportLocation,
        service: SmartHttpService,
    ) -> Result<SmartHttpAdvertisement> {
        let response = self.get_info_refs(location, service)?;
        SmartHttpAdvertisement::parse(service, &response.body)
    }

    /// Performs a smart HTTP upload-pack POST request.
    pub fn post_upload_pack(
        &self,
        location: &TransportLocation,
        request: &UploadPackRequest,
    ) -> Result<SmartHttpResponse> {
        let post_request = location.smart_http_upload_pack(request)?;
        let response = self.send_http_request(
            "POST",
            &post_request.url,
            Some(post_request.content_type.as_str()),
            &post_request.body,
        )?;
        validate_smart_http_response(
            &response,
            &post_request.response_content_type,
            &[200],
            SmartHttpBodyCheck::None,
        )?;
        Ok(response)
    }

    /// Performs a smart HTTP receive-pack POST request and parses report-status.
    pub fn post_receive_pack(
        &self,
        location: &TransportLocation,
        request: &ReceivePackRequest,
    ) -> Result<ReceivePackStatus> {
        let post_request = location.smart_http_receive_pack(request)?;
        let response = self.send_http_request(
            "POST",
            &post_request.url,
            Some(post_request.content_type.as_str()),
            &post_request.body,
        )?;
        validate_smart_http_response(
            &response,
            &post_request.response_content_type,
            &[200],
            SmartHttpBodyCheck::None,
        )?;
        ReceivePackStatus::parse(&response.body)
    }

    fn send_http_request(
        &self,
        method: &str,
        url: &str,
        content_type: Option<&str>,
        body: &[u8],
    ) -> Result<SmartHttpResponse> {
        let parsed_url = PlainHttpUrl::parse(url)?;
        let address = format!("{}:{}", parsed_url.host, parsed_url.port);
        let mut stream = TcpStream::connect(address)
            .map_err(|source| RitError::transport_io(url.to_owned(), source))?;
        stream
            .set_read_timeout(Some(self.timeout))
            .map_err(|source| RitError::transport_io(url.to_owned(), source))?;
        stream
            .set_write_timeout(Some(self.timeout))
            .map_err(|source| RitError::transport_io(url.to_owned(), source))?;

        let request = build_http_request(method, &parsed_url, content_type, body);
        stream
            .write_all(&request)
            .map_err(|source| RitError::transport_io(url.to_owned(), source))?;
        stream
            .flush()
            .map_err(|source| RitError::transport_io(url.to_owned(), source))?;

        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .map_err(|source| RitError::transport_io(url.to_owned(), source))?;
        parse_http_response(&response)
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

    /// Builds the smart HTTP upload-pack POST request metadata.
    pub fn smart_http_upload_pack(
        &self,
        request: &UploadPackRequest,
    ) -> Result<SmartHttpPostRequest> {
        self.smart_http_post_request(SmartHttpService::UploadPack, request.to_pkt_lines())
    }

    /// Builds the smart HTTP receive-pack POST request metadata.
    pub fn smart_http_receive_pack(
        &self,
        request: &ReceivePackRequest,
    ) -> Result<SmartHttpPostRequest> {
        self.smart_http_post_request(SmartHttpService::ReceivePack, request.to_bytes())
    }

    fn smart_http_post_request(
        &self,
        service: SmartHttpService,
        body: Vec<u8>,
    ) -> Result<SmartHttpPostRequest> {
        match self.protocol {
            TransportProtocol::Http | TransportProtocol::Https => {
                let base = self.original.trim_end_matches('/');
                let service_name = service.name();
                Ok(SmartHttpPostRequest {
                    url: format!("{base}/{service_name}"),
                    content_type: format!("application/x-{service_name}-request"),
                    response_content_type: format!("application/x-{service_name}-result"),
                    body,
                })
            }
            _ => Err(RitError::invalid_input(format!(
                "smart HTTP requires an HTTP(S) location: {}",
                self.original
            ))),
        }
    }

    /// Builds the remote Git service command for SSH transports.
    pub fn ssh_service_command(&self, service: SmartHttpService) -> Result<SshServiceCommand> {
        if self.protocol != TransportProtocol::Ssh {
            return Err(RitError::invalid_input(format!(
                "SSH transport requires an SSH location: {}",
                self.original
            )));
        }
        let parsed = ParsedSshLocation::parse(&self.original)?;
        let remote_command = format!("{} {}", service.name(), shell_quote(&parsed.path));
        Ok(SshServiceCommand {
            service,
            user: parsed.user,
            host: parsed.host,
            path: parsed.path,
            remote_command,
        })
    }
}

struct ParsedSshLocation {
    user: Option<String>,
    host: String,
    path: String,
}

impl ParsedSshLocation {
    fn parse(input: &str) -> Result<Self> {
        if let Some(rest) = input.strip_prefix("ssh://") {
            parse_ssh_url(rest, input)
        } else {
            parse_scp_like_location(input)
        }
    }
}

fn parse_ssh_url(rest: &str, original: &str) -> Result<ParsedSshLocation> {
    let Some((authority, path_without_slash)) = rest.split_once('/') else {
        return Err(RitError::invalid_input(format!(
            "SSH URL is missing repository path: {original}"
        )));
    };
    let (user, host) = parse_ssh_authority(authority, original)?;
    if path_without_slash.is_empty() {
        return Err(RitError::invalid_input(format!(
            "SSH URL is missing repository path: {original}"
        )));
    }
    Ok(ParsedSshLocation {
        user,
        host,
        path: format!("/{path_without_slash}"),
    })
}

fn parse_scp_like_location(input: &str) -> Result<ParsedSshLocation> {
    let Some((authority, path)) = input.split_once(':') else {
        return Err(RitError::invalid_input(format!(
            "unsupported SSH location: {input}"
        )));
    };
    let (user, host) = parse_ssh_authority(authority, input)?;
    if path.is_empty() {
        return Err(RitError::invalid_input(format!(
            "SSH location is missing repository path: {input}"
        )));
    }
    Ok(ParsedSshLocation {
        user,
        host,
        path: path.to_owned(),
    })
}

fn parse_ssh_authority(authority: &str, original: &str) -> Result<(Option<String>, String)> {
    if authority.is_empty() {
        return Err(RitError::invalid_input(format!(
            "SSH location is missing host: {original}"
        )));
    }
    let (user, host) = match authority.rsplit_once('@') {
        Some((user, host)) if !user.is_empty() && !host.is_empty() => {
            (Some(user.to_owned()), host.to_owned())
        }
        Some(_) => {
            return Err(RitError::invalid_input(format!(
                "SSH location has invalid user or host: {original}"
            )));
        }
        None => (None, authority.to_owned()),
    };
    Ok((user, host))
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

struct PlainHttpUrl {
    host: String,
    port: u16,
    path_and_query: String,
}

impl PlainHttpUrl {
    fn parse(url: &str) -> Result<Self> {
        let Some(rest) = url.strip_prefix("http://") else {
            return Err(RitError::invalid_input(format!(
                "blocking smart HTTP client currently supports only plain http:// URLs: {url}"
            )));
        };
        let (host_port, path_and_query) = match rest.split_once('/') {
            Some((host_port, path)) => (host_port, format!("/{path}")),
            None => (rest, "/".to_owned()),
        };
        if host_port.is_empty() {
            return Err(RitError::invalid_input(format!(
                "HTTP URL is missing a host: {url}"
            )));
        }
        let (host, port) = match host_port.rsplit_once(':') {
            Some((host, port)) if !host.is_empty() => {
                let port = port.parse::<u16>().map_err(|_| {
                    RitError::invalid_input(format!("HTTP URL has invalid port: {url}"))
                })?;
                (host.to_owned(), port)
            }
            _ => (host_port.to_owned(), 80),
        };
        Ok(Self {
            host,
            port,
            path_and_query,
        })
    }

    fn host_header(&self) -> String {
        if self.port == 80 {
            self.host.clone()
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

fn build_http_request(
    method: &str,
    url: &PlainHttpUrl,
    content_type: Option<&str>,
    body: &[u8],
) -> Vec<u8> {
    let mut request = format!(
        "{method} {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: rit/{}\r\nAccept: */*\r\nConnection: close\r\n",
        url.path_and_query,
        url.host_header(),
        crate::version()
    )
    .into_bytes();
    if let Some(content_type) = content_type {
        request.extend_from_slice(format!("Content-Type: {content_type}\r\n").as_bytes());
        request.extend_from_slice(format!("Content-Length: {}\r\n", body.len()).as_bytes());
    }
    request.extend_from_slice(b"\r\n");
    request.extend_from_slice(body);
    request
}

fn parse_http_response(bytes: &[u8]) -> Result<SmartHttpResponse> {
    let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") else {
        return Err(RitError::invalid_input(
            "HTTP response is missing header terminator",
        ));
    };
    let header_bytes = &bytes[..header_end];
    let raw_body = &bytes[header_end + 4..];
    let header_text = std::str::from_utf8(header_bytes)
        .map_err(|_| RitError::invalid_input("HTTP response headers are not UTF-8"))?;
    let mut lines = header_text.split("\r\n");
    let Some(status_line) = lines.next() else {
        return Err(RitError::invalid_input("HTTP response is empty"));
    };
    let status_code = parse_http_status_code(status_line)?;
    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let Some((name, value)) = line.split_once(':') else {
            return Err(RitError::invalid_input("malformed HTTP response header"));
        };
        headers.push((name.trim().to_owned(), value.trim().to_owned()));
    }

    let body = if http_response_is_chunked(&headers) {
        decode_chunked_body(raw_body)?
    } else {
        raw_body.to_vec()
    };

    Ok(SmartHttpResponse {
        status_code,
        headers,
        body,
    })
}

fn http_response_is_chunked(headers: &[(String, String)]) -> bool {
    headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("Transfer-Encoding"))
        .map(|(_, value)| value.to_ascii_lowercase().contains("chunked"))
        .unwrap_or(false)
}

fn decode_chunked_body(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut position = 0;
    loop {
        let size_line_end = find_crlf(bytes, position)
            .ok_or_else(|| RitError::invalid_input("chunked response is missing chunk size"))?;
        let size_line = std::str::from_utf8(&bytes[position..size_line_end])
            .map_err(|_| RitError::invalid_input("chunked response size is not UTF-8"))?;
        let size_text = size_line.split(';').next().unwrap_or(size_line).trim();
        let chunk_size = usize::from_str_radix(size_text, 16)
            .map_err(|_| RitError::invalid_input("chunked response has invalid chunk size"))?;
        position = size_line_end + 2;

        if chunk_size == 0 {
            return consume_chunked_trailers(bytes, position).map(|_| output);
        }

        let chunk_end = position + chunk_size;
        let Some(chunk) = bytes.get(position..chunk_end) else {
            return Err(RitError::invalid_input(
                "chunked response body is truncated",
            ));
        };
        output.extend_from_slice(chunk);
        position = chunk_end;

        if bytes.get(position..position + 2) != Some(b"\r\n") {
            return Err(RitError::invalid_input(
                "chunked response chunk is missing terminator",
            ));
        }
        position += 2;
    }
}

fn consume_chunked_trailers(bytes: &[u8], mut position: usize) -> Result<usize> {
    loop {
        let line_end = find_crlf(bytes, position)
            .ok_or_else(|| RitError::invalid_input("chunked response trailers are truncated"))?;
        if line_end == position {
            return Ok(line_end + 2);
        }
        position = line_end + 2;
    }
}

fn find_crlf(bytes: &[u8], start: usize) -> Option<usize> {
    bytes
        .get(start..)?
        .windows(2)
        .position(|window| window == b"\r\n")
        .map(|offset| start + offset)
}

fn parse_http_status_code(status_line: &str) -> Result<u16> {
    let mut parts = status_line.split_whitespace();
    let Some(version) = parts.next() else {
        return Err(RitError::invalid_input("HTTP response is missing version"));
    };
    if !version.starts_with("HTTP/") {
        return Err(RitError::invalid_input(
            "HTTP response has invalid status line",
        ));
    }
    let Some(code) = parts.next() else {
        return Err(RitError::invalid_input(
            "HTTP response is missing status code",
        ));
    };
    code.parse::<u16>()
        .map_err(|_| RitError::invalid_input("HTTP response has invalid status code"))
}

enum SmartHttpBodyCheck {
    None,
    InfoRefsAdvertisement,
}

fn validate_smart_http_response(
    response: &SmartHttpResponse,
    expected_content_type: &str,
    allowed_status_codes: &[u16],
    body_check: SmartHttpBodyCheck,
) -> Result<()> {
    if !allowed_status_codes.contains(&response.status_code) {
        return Err(RitError::invalid_input(format!(
            "smart HTTP response returned unexpected status code: {}",
            response.status_code
        )));
    }
    validate_smart_http_content_type(response, expected_content_type)?;
    match body_check {
        SmartHttpBodyCheck::None => {}
        SmartHttpBodyCheck::InfoRefsAdvertisement => {
            validate_info_refs_advertisement_prefix(&response.body)?;
        }
    }
    Ok(())
}

fn validate_smart_http_content_type(
    response: &SmartHttpResponse,
    expected_content_type: &str,
) -> Result<()> {
    let Some(content_type) = response.header("Content-Type") else {
        return Err(RitError::invalid_input(
            "smart HTTP response is missing Content-Type",
        ));
    };
    let media_type = content_type
        .split(';')
        .next()
        .unwrap_or(content_type)
        .trim();
    if media_type != expected_content_type {
        return Err(RitError::invalid_input(format!(
            "smart HTTP response has unsupported Content-Type: {content_type}"
        )));
    }
    Ok(())
}

fn validate_info_refs_advertisement_prefix(body: &[u8]) -> Result<()> {
    let Some(prefix) = body.get(..5) else {
        return Err(RitError::invalid_input(
            "smart HTTP info/refs response is too short",
        ));
    };
    if prefix[..4].iter().all(u8::is_ascii_hexdigit) && prefix[4] == b'#' {
        Ok(())
    } else {
        Err(RitError::invalid_input(
            "smart HTTP info/refs response does not start with an advertisement pkt-line",
        ))
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
        BlockingSmartHttpClient, FetchRefSpec, SmartHttpAdvertisement, SmartHttpService,
        TransportLocation, TransportProtocol, UploadPackAckStatus, UploadPackAcknowledgement,
        UploadPackResponse, UploadPackSideBand,
    };
    use crate::ObjectId;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
        time::Duration,
    };

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
    fn builds_ssh_service_command_for_scp_like_locations() {
        let command = TransportLocation::parse("git@example.test:org/repo.git")
            .ssh_service_command(SmartHttpService::UploadPack)
            .expect("ssh command");

        assert_eq!(command.service, SmartHttpService::UploadPack);
        assert_eq!(command.user.as_deref(), Some("git"));
        assert_eq!(command.host, "example.test");
        assert_eq!(command.path, "org/repo.git");
        assert_eq!(command.remote_command, "git-upload-pack 'org/repo.git'");
    }

    #[test]
    fn builds_ssh_service_command_for_ssh_urls() {
        let command = TransportLocation::parse("ssh://git@example.test/project.git")
            .ssh_service_command(SmartHttpService::ReceivePack)
            .expect("ssh command");

        assert_eq!(command.service, SmartHttpService::ReceivePack);
        assert_eq!(command.user.as_deref(), Some("git"));
        assert_eq!(command.host, "example.test");
        assert_eq!(command.path, "/project.git");
        assert_eq!(command.remote_command, "git-receive-pack '/project.git'");
    }

    #[test]
    fn quotes_ssh_repository_paths_for_remote_shells() {
        let command = TransportLocation::parse("example.test:org/repo'with-quote.git")
            .ssh_service_command(SmartHttpService::UploadPack)
            .expect("ssh command");

        assert_eq!(
            command.remote_command,
            "git-upload-pack 'org/repo'\\''with-quote.git'"
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
    fn builds_smart_http_upload_pack_requests() {
        let want = ObjectId::from_hex("0a53e9ddeaddad63ad106860237bbf53411d11a7").expect("want");
        let upload_pack = super::UploadPackRequest::new(vec![want]).expect("request");
        let location = TransportLocation::parse("https://example.test/repo.git/");
        let request = location
            .smart_http_upload_pack(&upload_pack)
            .expect("https supports smart http metadata");

        assert_eq!(request.url, "https://example.test/repo.git/git-upload-pack");
        assert_eq!(
            request.content_type,
            "application/x-git-upload-pack-request"
        );
        assert_eq!(
            request.response_content_type,
            "application/x-git-upload-pack-result"
        );
        assert_eq!(request.body, upload_pack.to_pkt_lines());
    }

    #[test]
    fn builds_smart_http_receive_pack_requests() {
        let old_id = ObjectId::from_bytes([0; 20]);
        let new_id = ObjectId::from_hex("0a53e9ddeaddad63ad106860237bbf53411d11a7").expect("new");
        let command =
            super::ReceivePackCommand::new(old_id, new_id, "refs/heads/main").expect("command");
        let receive_pack = super::ReceivePackRequest::new(vec![command]).expect("request");
        let location = TransportLocation::parse("https://example.test/repo.git/");
        let request = location
            .smart_http_receive_pack(&receive_pack)
            .expect("https supports smart http metadata");

        assert_eq!(
            request.url,
            "https://example.test/repo.git/git-receive-pack"
        );
        assert_eq!(
            request.content_type,
            "application/x-git-receive-pack-request"
        );
        assert_eq!(
            request.response_content_type,
            "application/x-git-receive-pack-result"
        );
        assert_eq!(request.body, receive_pack.to_bytes());
    }

    #[test]
    fn blocking_http_client_gets_info_refs() {
        let (base_url, request_handle) = serve_one_http_request(
            b"HTTP/1.1 200 OK\r\nContent-Type: application/x-git-upload-pack-advertisement\r\nContent-Length: 34\r\nConnection: close\r\n\r\n001e# service=git-upload-pack\n0000",
        );
        let location = TransportLocation::parse(&format!("{base_url}/repo.git"));
        let client = BlockingSmartHttpClient::new(Duration::from_secs(2));
        let response = client
            .get_info_refs(&location, SmartHttpService::UploadPack)
            .expect("GET should succeed");
        let request = String::from_utf8(request_handle.join().expect("server thread"))
            .expect("request is UTF-8");

        assert!(
            request.starts_with("GET /repo.git/info/refs?service=git-upload-pack HTTP/1.1\r\n")
        );
        assert!(request.contains("\r\nHost: 127.0.0.1:"));
        assert_eq!(response.status_code, 200);
        assert_eq!(
            response.header("content-type"),
            Some("application/x-git-upload-pack-advertisement")
        );
        assert_eq!(response.body, b"001e# service=git-upload-pack\n0000");
    }

    #[test]
    fn blocking_http_client_posts_upload_pack() {
        let (base_url, request_handle) =
            serve_one_http_request(b"HTTP/1.1 200 OK\r\nContent-Type: application/x-git-upload-pack-result\r\nContent-Length: 8\r\n\r\n0008NAK\n");
        let location = TransportLocation::parse(&format!("{base_url}/repo.git"));
        let want = ObjectId::from_hex("0a53e9ddeaddad63ad106860237bbf53411d11a7").expect("want");
        let upload_pack = super::UploadPackRequest::new(vec![want]).expect("request");
        let client = BlockingSmartHttpClient::new(Duration::from_secs(2));
        let response = client
            .post_upload_pack(&location, &upload_pack)
            .expect("POST should succeed");
        let request = request_handle.join().expect("server thread");
        let request_text =
            String::from_utf8(request.clone()).expect("request headers and body are UTF-8");

        assert!(request_text.starts_with("POST /repo.git/git-upload-pack HTTP/1.1\r\n"));
        assert!(
            request_text.contains("\r\nContent-Type: application/x-git-upload-pack-request\r\n")
        );
        assert!(request.ends_with(&upload_pack.to_pkt_lines()));
        assert_eq!(response.body, b"0008NAK\n");
    }

    #[test]
    fn blocking_http_client_posts_receive_pack_and_parses_status() {
        let (base_url, request_handle) = serve_one_http_request(
            b"HTTP/1.1 200 OK\r\nContent-Type: application/x-git-receive-pack-result\r\nConnection: close\r\n\r\n000eunpack ok\n0017ok refs/heads/main\n0000",
        );
        let location = TransportLocation::parse(&format!("{base_url}/repo.git"));
        let old_id = ObjectId::from_bytes([0; 20]);
        let new_id = ObjectId::from_hex("0a53e9ddeaddad63ad106860237bbf53411d11a7").expect("new");
        let command =
            super::ReceivePackCommand::new(old_id, new_id, "refs/heads/main").expect("command");
        let receive_pack = super::ReceivePackRequest::new(vec![command])
            .expect("request")
            .with_capabilities(vec!["report-status".to_owned()]);
        let client = BlockingSmartHttpClient::new(Duration::from_secs(2));
        let status = client
            .post_receive_pack(&location, &receive_pack)
            .expect("POST should succeed");
        let request = request_handle.join().expect("server thread");
        let request_text =
            String::from_utf8(request.clone()).expect("request headers and body are UTF-8");

        assert!(request_text.starts_with("POST /repo.git/git-receive-pack HTTP/1.1\r\n"));
        assert!(
            request_text.contains("\r\nContent-Type: application/x-git-receive-pack-request\r\n")
        );
        assert!(request.ends_with(&receive_pack.to_bytes()));
        assert_eq!(status.unpack_error, None);
        assert_eq!(
            status.commands,
            [super::ReceivePackCommandStatus::Ok {
                ref_name: "refs/heads/main".to_owned(),
            }]
        );
    }

    #[test]
    fn blocking_http_client_decodes_chunked_responses() {
        let (base_url, request_handle) = serve_one_http_request(
            b"HTTP/1.1 200 OK\r\nContent-Type: application/x-git-upload-pack-advertisement\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n22\r\n001e# service=git-upload-pack\n0000\r\n0\r\n\r\n",
        );
        let location = TransportLocation::parse(&format!("{base_url}/repo.git"));
        let client = BlockingSmartHttpClient::new(Duration::from_secs(2));
        let response = client
            .get_info_refs(&location, SmartHttpService::UploadPack)
            .expect("GET should succeed");

        request_handle.join().expect("server thread");
        assert_eq!(response.status_code, 200);
        assert_eq!(response.body, b"001e# service=git-upload-pack\n0000");
    }

    #[test]
    fn blocking_http_client_rejects_wrong_content_type() {
        let (base_url, request_handle) = serve_one_http_request(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 34\r\n\r\n001e# service=git-upload-pack\n0000",
        );
        let location = TransportLocation::parse(&format!("{base_url}/repo.git"));
        let client = BlockingSmartHttpClient::new(Duration::from_secs(2));
        let error = client
            .get_info_refs(&location, SmartHttpService::UploadPack)
            .expect_err("content type should be validated");

        request_handle.join().expect("server thread");
        assert!(error.to_string().contains("Content-Type"));
    }

    #[test]
    fn blocking_http_client_rejects_bad_info_refs_prefix() {
        let (base_url, request_handle) = serve_one_http_request(
            b"HTTP/1.1 200 OK\r\nContent-Type: application/x-git-upload-pack-advertisement\r\nContent-Length: 5\r\n\r\nhello",
        );
        let location = TransportLocation::parse(&format!("{base_url}/repo.git"));
        let client = BlockingSmartHttpClient::new(Duration::from_secs(2));
        let error = client
            .get_info_refs(&location, SmartHttpService::UploadPack)
            .expect_err("advertisement prefix should be validated");

        request_handle.join().expect("server thread");
        assert!(error.to_string().contains("advertisement pkt-line"));
    }

    #[test]
    fn blocking_http_client_discovers_advertised_refs() {
        let (base_url, request_handle) = serve_one_http_request(
            concat!(
                "HTTP/1.1 200 OK\r\n",
                "Content-Type: application/x-git-upload-pack-advertisement\r\n",
                "Connection: close\r\n",
                "\r\n",
                "001e# service=git-upload-pack\n",
                "0000",
                "005195dcfa3633004da0049d3d0fa03f80589cbcaf31 refs/heads/main\0multi_ack thin-pack\n",
                "0000"
            )
            .as_bytes(),
        );
        let location = TransportLocation::parse(&format!("{base_url}/repo.git"));
        let client = BlockingSmartHttpClient::new(Duration::from_secs(2));
        let advertisement = client
            .discover_refs(&location, SmartHttpService::UploadPack)
            .expect("refs should parse");

        request_handle.join().expect("server thread");
        assert_eq!(advertisement.service, SmartHttpService::UploadPack);
        assert_eq!(advertisement.capabilities, ["multi_ack", "thin-pack"]);
        assert_eq!(advertisement.refs.len(), 1);
        assert_eq!(advertisement.refs[0].name, "refs/heads/main");
    }

    #[test]
    fn rejects_truncated_chunked_responses() {
        let error = super::parse_http_response(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nbo",
        )
        .expect_err("chunk is truncated");

        assert!(error.to_string().contains("truncated"));
    }

    #[test]
    fn blocking_http_client_rejects_https_until_tls_exists() {
        let location = TransportLocation::parse("https://example.test/repo.git");
        let client = BlockingSmartHttpClient::new(Duration::from_secs(2));
        let error = client
            .get_info_refs(&location, SmartHttpService::UploadPack)
            .expect_err("https needs TLS support");

        assert!(error.to_string().contains("plain http://"));
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
    fn builds_receive_pack_request_body() {
        let old_id = ObjectId::from_hex("441b40d833fdfa93eb2908e52742248faf0ee993").expect("old");
        let new_id = ObjectId::from_hex("0a53e9ddeaddad63ad106860237bbf53411d11a7").expect("new");
        let command =
            super::ReceivePackCommand::new(old_id, new_id, "refs/heads/main").expect("command");
        let request = super::ReceivePackRequest::new(vec![command])
            .expect("request")
            .with_capabilities(vec!["report-status".to_owned(), "side-band-64k".to_owned()])
            .with_pack_data(b"PACKdata".to_vec());

        let body = request.to_bytes();
        let command_bytes = &body[..body.len() - b"PACKdata".len()];
        let command_text = String::from_utf8(command_bytes.to_vec()).expect("command text");

        assert!(command_text.contains(
            "441b40d833fdfa93eb2908e52742248faf0ee993 \
             0a53e9ddeaddad63ad106860237bbf53411d11a7 \
             refs/heads/main\0report-status side-band-64k\n"
        ));
        assert!(command_text.ends_with("0000"));
        assert!(body.ends_with(b"PACKdata"));
    }

    #[test]
    fn rejects_empty_receive_pack_requests() {
        let error = super::ReceivePackRequest::new(Vec::new()).expect_err("command is required");

        assert!(error.to_string().contains("at least one command"));
    }

    #[test]
    fn rejects_receive_pack_commands_without_ref_names() {
        let old_id = ObjectId::from_bytes([0; 20]);
        let new_id = ObjectId::from_hex("0a53e9ddeaddad63ad106860237bbf53411d11a7").expect("new");
        let error =
            super::ReceivePackCommand::new(old_id, new_id, "").expect_err("ref is required");

        assert!(error.to_string().contains("ref name"));
    }

    #[test]
    fn parses_receive_pack_status_reports() {
        let status = super::ReceivePackStatus::parse(
            concat!(
                "000eunpack ok\n",
                "0018ok refs/heads/debug\n",
                "002ang refs/heads/master non-fast-forward\n",
                "0000"
            )
            .as_bytes(),
        )
        .expect("status");

        assert_eq!(status.unpack_error, None);
        assert_eq!(
            status.commands,
            [
                super::ReceivePackCommandStatus::Ok {
                    ref_name: "refs/heads/debug".to_owned(),
                },
                super::ReceivePackCommandStatus::Rejected {
                    ref_name: "refs/heads/master".to_owned(),
                    message: "non-fast-forward".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn parses_receive_pack_unpack_errors() {
        let status = super::ReceivePackStatus::parse(
            b"001dunpack index-pack failed\n0017ok refs/heads/main\n0000",
        )
        .expect("status");

        assert_eq!(status.unpack_error, Some("index-pack failed".to_owned()));
    }

    #[test]
    fn rejects_receive_pack_status_without_command_results() {
        let error =
            super::ReceivePackStatus::parse(b"000eunpack ok\n0000").expect_err("command required");

        assert!(error.to_string().contains("no command results"));
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

    fn serve_one_http_request(response: &'static [u8]) -> (String, thread::JoinHandle<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let address = listener.local_addr().expect("local address");
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept connection");
            let mut request = Vec::new();
            let mut buffer = [0_u8; 1024];
            loop {
                let bytes_read = stream.read(&mut buffer).expect("read request");
                if bytes_read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..bytes_read]);
                if request_is_complete(&request) {
                    break;
                }
            }
            stream.write_all(response).expect("write response");
            request
        });
        (format!("http://127.0.0.1:{}", address.port()), handle)
    }

    fn request_is_complete(request: &[u8]) -> bool {
        let Some(header_end) = request
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .map(|index| index + 4)
        else {
            return false;
        };
        let header_text = String::from_utf8_lossy(&request[..header_end]);
        let Some(content_length) = header_text.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("Content-Length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        }) else {
            return true;
        };
        request.len() >= header_end + content_length
    }
}
