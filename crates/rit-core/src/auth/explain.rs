use super::CredentialRequest;
use crate::transport::{TransportLocation, TransportProtocol};
use std::env;

/// Read-only explanation of how `rit` would approach authentication for a
/// repository location.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthExplanation {
    /// Original user-provided location.
    pub location: String,
    /// Transport protocol inferred from the location.
    pub protocol: AuthProtocol,
    /// Credential request that would be used by an auth provider, when one is
    /// relevant.
    pub credential_request: Option<CredentialRequest>,
    /// Environment token variables that are currently populated.
    pub available_environment_tokens: Vec<String>,
    /// Human-readable notes that explain provider selection without exposing
    /// secret values.
    pub notes: Vec<String>,
}

impl AuthExplanation {
    /// Returns true when the location can require a credential or SSH identity.
    pub fn uses_credentials(&self) -> bool {
        self.credential_request.is_some()
    }
}

/// Protocol label used by auth explanations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthProtocol {
    /// Local filesystem path.
    Local,
    /// Plain HTTP remote.
    Http,
    /// HTTPS remote.
    Https,
    /// SSH remote, including scp-like locations.
    Ssh,
}

impl AuthProtocol {
    /// Stable display label for CLI output and JSON consumers.
    pub fn label(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Http => "http",
            Self::Https => "https",
            Self::Ssh => "ssh",
        }
    }
}

/// Builds an auth explanation from the current process environment.
pub fn explain_auth_location(location: &str) -> AuthExplanation {
    let env_values = super::DEFAULT_TOKEN_ENV_VARS.iter().map(|variable| {
        let value = env::var(variable).ok();
        ((*variable).to_owned(), value)
    });
    explain_auth_location_with_env(location, env_values)
}

/// Builds an auth explanation with caller-provided environment values.
///
/// The values are inspected only for presence; secret contents are never stored
/// in the returned explanation.
pub fn explain_auth_location_with_env(
    location: &str,
    env_values: impl IntoIterator<Item = (String, Option<String>)>,
) -> AuthExplanation {
    let transport = TransportLocation::parse(location);
    let protocol = AuthProtocol::from(transport.protocol());
    let available_environment_tokens = env_values
        .into_iter()
        .filter_map(|(variable, value)| value.filter(|token| !token.is_empty()).map(|_| variable))
        .collect::<Vec<_>>();
    let credential_request = credential_request_for_location(location, transport.protocol());
    let mut notes = notes_for(protocol, &available_environment_tokens);

    if credential_request.is_none() && protocol != AuthProtocol::Local {
        notes.push("rit could not parse a host from this remote location".to_owned());
    }

    AuthExplanation {
        location: location.to_owned(),
        protocol,
        credential_request,
        available_environment_tokens,
        notes,
    }
}

fn credential_request_for_location(
    location: &str,
    protocol: TransportProtocol,
) -> Option<CredentialRequest> {
    match protocol {
        TransportProtocol::Local => None,
        TransportProtocol::Http => parse_http_like_location(location, "http"),
        TransportProtocol::Https => parse_http_like_location(location, "https"),
        TransportProtocol::Ssh => parse_ssh_like_location(location),
    }
}

fn parse_http_like_location(location: &str, protocol: &str) -> Option<CredentialRequest> {
    let rest = location.strip_prefix(&format!("{protocol}://"))?;
    let (authority, path) = split_authority_and_path(rest);
    let (username, host) = split_user_and_host(authority)?;
    let mut request = CredentialRequest::new(protocol, host);
    request.username = username;
    request.path = path;
    Some(request)
}

fn parse_ssh_like_location(location: &str) -> Option<CredentialRequest> {
    if let Some(rest) = location.strip_prefix("ssh://") {
        let (authority, path) = split_authority_and_path(rest);
        let (username, host) = split_user_and_host(authority)?;
        let mut request = CredentialRequest::new("ssh", strip_port(&host));
        request.username = username;
        request.path = path;
        return Some(request);
    }

    let (authority, path) = location.split_once(':')?;
    let (username, host) = split_user_and_host(authority)?;
    let mut request = CredentialRequest::new("ssh", host);
    request.username = username;
    request.path = (!path.is_empty()).then(|| path.to_owned());
    Some(request)
}

fn split_authority_and_path(rest: &str) -> (&str, Option<String>) {
    match rest.split_once('/') {
        Some((authority, path)) if !path.is_empty() => (authority, Some(path.to_owned())),
        Some((authority, _)) => (authority, None),
        None => (rest, None),
    }
}

fn split_user_and_host(authority: &str) -> Option<(Option<String>, String)> {
    let authority_without_query = authority.split('?').next().unwrap_or(authority);
    let (username, host) = match authority_without_query.rsplit_once('@') {
        Some((username, host)) if !username.is_empty() && !host.is_empty() => {
            (Some(username.to_owned()), host)
        }
        Some(_) => return None,
        None => (None, authority_without_query),
    };
    let host = strip_port(host);
    (!host.is_empty()).then_some((username, host))
}

fn strip_port(host: &str) -> String {
    let Some((name, port)) = host.rsplit_once(':') else {
        return host.to_owned();
    };
    if name.is_empty() || port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()) {
        return host.to_owned();
    }
    name.to_owned()
}

fn notes_for(protocol: AuthProtocol, available_environment_tokens: &[String]) -> Vec<String> {
    let mut notes = Vec::new();
    match protocol {
        AuthProtocol::Local => {
            notes.push("local paths do not need rit credential lookup".to_owned());
        }
        AuthProtocol::Http => {
            notes.push("plain HTTP can use credential helpers, but HTTPS is preferred".to_owned());
        }
        AuthProtocol::Https => {
            notes.push(
                "HTTPS remotes can use environment tokens or Git credential helpers".to_owned(),
            );
        }
        AuthProtocol::Ssh => {
            notes.push(
                "SSH remotes use SSH keys or ssh-agent identities instead of HTTP tokens"
                    .to_owned(),
            );
        }
    }
    if available_environment_tokens.is_empty() {
        notes.push("no default rit token environment variables are set".to_owned());
    } else {
        notes.push("environment token values are redacted and never printed".to_owned());
    }
    notes
}

impl From<TransportProtocol> for AuthProtocol {
    fn from(protocol: TransportProtocol) -> Self {
        match protocol {
            TransportProtocol::Local => Self::Local,
            TransportProtocol::Http => Self::Http,
            TransportProtocol::Https => Self::Https,
            TransportProtocol::Ssh => Self::Ssh,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_explain_parses_https_without_storing_secret_values() {
        let explanation = explain_auth_location_with_env(
            "https://alice@example.test/org/repo.git",
            vec![
                (
                    "GITHUB_TOKEN".to_owned(),
                    Some("super-secret-token".to_owned()),
                ),
                ("RIT_TOKEN".to_owned(), None),
            ],
        );

        let request = explanation
            .credential_request
            .as_ref()
            .expect("https should use a credential request");
        assert_eq!(explanation.protocol, AuthProtocol::Https);
        assert_eq!(request.protocol, "https");
        assert_eq!(request.host, "example.test");
        assert_eq!(request.path.as_deref(), Some("org/repo.git"));
        assert_eq!(request.username.as_deref(), Some("alice"));
        assert_eq!(explanation.available_environment_tokens, ["GITHUB_TOKEN"]);
        assert!(!format!("{explanation:?}").contains("super-secret-token"));
    }

    #[test]
    fn auth_explain_parses_scp_like_ssh_location() {
        let explanation =
            explain_auth_location_with_env("git@example.test:org/repo.git", Vec::new());

        let request = explanation
            .credential_request
            .as_ref()
            .expect("ssh should use a credential request");
        assert_eq!(explanation.protocol, AuthProtocol::Ssh);
        assert_eq!(request.protocol, "ssh");
        assert_eq!(request.host, "example.test");
        assert_eq!(request.path.as_deref(), Some("org/repo.git"));
        assert_eq!(request.username.as_deref(), Some("git"));
    }

    #[test]
    fn auth_explain_keeps_local_paths_credential_free() {
        let explanation = explain_auth_location_with_env("../repo.git", Vec::new());

        assert_eq!(explanation.protocol, AuthProtocol::Local);
        assert_eq!(explanation.credential_request, None);
        assert!(!explanation.uses_credentials());
    }
}
