use std::path::PathBuf;

use crate::{Result, RitError};

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
    use super::{FetchRefSpec, SmartHttpService, TransportLocation, TransportProtocol};

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
}
