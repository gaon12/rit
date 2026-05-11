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
        let length_end = position + 4;
        let Some(length_bytes) = bytes.get(position..length_end) else {
            return Err(RitError::invalid_input("truncated pkt-line length"));
        };
        let length_text = std::str::from_utf8(length_bytes)
            .map_err(|_| RitError::invalid_input("pkt-line length is not UTF-8"))?;
        let length = u16::from_str_radix(length_text, 16)
            .map_err(|_| RitError::invalid_input("invalid pkt-line length"))?
            as usize;
        position = length_end;

        if length == 0 {
            lines.push(Vec::new());
            continue;
        }
        if length < 4 {
            return Err(RitError::invalid_input(
                "pkt-line length is smaller than header",
            ));
        }
        let payload_len = length - 4;
        let payload_end = position + payload_len;
        let Some(payload) = bytes.get(position..payload_end) else {
            return Err(RitError::invalid_input("truncated pkt-line payload"));
        };
        lines.push(payload.to_vec());
        position = payload_end;
    }
    Ok(lines)
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
        TransportProtocol,
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
}
