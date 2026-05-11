use crate::Result;
use std::fmt::{Debug, Display, Formatter};

/// Secret text that must not leak through formatting.
#[derive(Clone, Eq, PartialEq)]
pub struct SecretString(String);

impl SecretString {
    /// Wraps a secret value.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the secret for callers that are about to authenticate.
    pub fn expose_secret(&self) -> &str {
        &self.0
    }

    /// Returns true when the secret is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Debug for SecretString {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("<redacted>")
    }
}

impl Display for SecretString {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("<redacted>")
    }
}

/// Credential material returned by auth providers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Credential {
    /// Optional username or token owner.
    pub username: Option<String>,
    /// Secret token, password, or key material.
    pub secret: SecretString,
    /// Source/type of credential.
    pub kind: CredentialKind,
}

impl Credential {
    /// Builds a token credential without a username.
    pub fn token(secret: impl Into<String>) -> Self {
        Self {
            username: None,
            secret: SecretString::new(secret),
            kind: CredentialKind::Token,
        }
    }

    /// Builds a username/password credential.
    pub fn password(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: Some(username.into()),
            secret: SecretString::new(password),
            kind: CredentialKind::Password,
        }
    }
}

/// Credential category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialKind {
    /// Bearer or personal access token.
    Token,
    /// Username/password pair.
    Password,
    /// SSH key material or agent identity.
    SshKey,
}

/// Auth lookup request shared by transport implementations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialRequest {
    /// URL scheme or transport protocol, such as `https` or `ssh`.
    pub protocol: String,
    /// Remote host.
    pub host: String,
    /// Optional path/repository name.
    pub path: Option<String>,
    /// Optional requested username.
    pub username: Option<String>,
}

impl CredentialRequest {
    /// Creates a credential request for one protocol and host.
    pub fn new(protocol: impl Into<String>, host: impl Into<String>) -> Self {
        Self {
            protocol: protocol.into(),
            host: host.into(),
            path: None,
            username: None,
        }
    }
}

/// Credential source abstraction used by transports.
pub trait CredentialProvider {
    /// Returns a matching credential or `None` when this provider has no answer.
    fn credential(&self, request: &CredentialRequest) -> Result<Option<Credential>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_are_redacted_in_debug_and_display() {
        let credential = Credential::token("super-secret-token");

        assert!(!format!("{credential:?}").contains("super-secret-token"));
        assert_eq!(credential.secret.to_string(), "<redacted>");
        assert_eq!(format!("{:?}", credential.secret), "<redacted>");
        assert_eq!(credential.secret.expose_secret(), "super-secret-token");
    }

    #[test]
    fn credential_request_records_remote_parts() {
        let mut request = CredentialRequest::new("https", "example.test");
        request.path = Some("org/repo.git".to_owned());
        request.username = Some("alice".to_owned());

        assert_eq!(request.protocol, "https");
        assert_eq!(request.host, "example.test");
        assert_eq!(request.path.as_deref(), Some("org/repo.git"));
        assert_eq!(request.username.as_deref(), Some("alice"));
    }
}
