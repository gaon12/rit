use super::{Credential, CredentialRequest, SecretString};
use crate::GitConfig;

/// Git credential helper line-protocol message.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GitCredentialMessage {
    /// Protocol such as `https`.
    pub protocol: Option<String>,
    /// Remote host.
    pub host: Option<String>,
    /// Remote path.
    pub path: Option<String>,
    /// Username.
    pub username: Option<String>,
    /// Password or token.
    pub password: Option<SecretString>,
}

/// Configured Git credential helper command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitCredentialHelper {
    /// Raw `credential.helper` value.
    pub command: String,
}

impl GitCredentialHelper {
    /// Reads the last configured `credential.helper`, matching scalar config
    /// lookup behavior used elsewhere in rit.
    pub fn from_config(config: &GitConfig) -> Option<Self> {
        config.get("credential", "helper").map(|helper| Self {
            command: helper.to_owned(),
        })
    }
}

impl GitCredentialMessage {
    /// Creates a helper request from a credential lookup request.
    pub fn from_request(request: &CredentialRequest) -> Self {
        Self {
            protocol: Some(request.protocol.clone()),
            host: Some(request.host.clone()),
            path: request.path.clone(),
            username: request.username.clone(),
            password: None,
        }
    }

    /// Creates a helper response-like message from a credential.
    pub fn from_credential(credential: &Credential) -> Self {
        Self {
            protocol: None,
            host: None,
            path: None,
            username: credential.username.clone(),
            password: Some(credential.secret.clone()),
        }
    }

    /// Encodes the message using Git's credential helper line protocol.
    pub fn to_protocol_text(&self) -> String {
        let mut output = String::new();
        push_protocol_field(&mut output, "protocol", self.protocol.as_deref());
        push_protocol_field(&mut output, "host", self.host.as_deref());
        push_protocol_field(&mut output, "path", self.path.as_deref());
        push_protocol_field(&mut output, "username", self.username.as_deref());
        push_protocol_field(
            &mut output,
            "password",
            self.password.as_ref().map(SecretString::expose_secret),
        );
        output.push('\n');
        output
    }

    /// Parses Git credential helper line protocol.
    pub fn parse_protocol_text(input: &str) -> Self {
        let mut message = Self::default();
        for line in input.lines() {
            if line.is_empty() {
                break;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key {
                "protocol" => message.protocol = Some(value.to_owned()),
                "host" => message.host = Some(value.to_owned()),
                "path" => message.path = Some(value.to_owned()),
                "username" => message.username = Some(value.to_owned()),
                "password" => message.password = Some(SecretString::new(value)),
                _ => {}
            }
        }
        message
    }
}

fn push_protocol_field(output: &mut String, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        output.push_str(key);
        output.push('=');
        output.push_str(value);
        output.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_credential_protocol_round_trips_without_debug_secret_leaks() {
        let mut request = CredentialRequest::new("https", "example.test");
        request.path = Some("org/repo.git".to_owned());
        request.username = Some("alice".to_owned());
        let request_message = GitCredentialMessage::from_request(&request);

        assert_eq!(
            request_message.to_protocol_text(),
            "protocol=https\nhost=example.test\npath=org/repo.git\nusername=alice\n\n"
        );

        let response = GitCredentialMessage::parse_protocol_text(
            "username=alice\npassword=super-secret-token\n\n",
        );

        assert_eq!(response.username.as_deref(), Some("alice"));
        assert_eq!(
            response
                .password
                .as_ref()
                .expect("password should parse")
                .expose_secret(),
            "super-secret-token"
        );
        assert!(!format!("{response:?}").contains("super-secret-token"));
    }

    #[test]
    fn git_credential_helper_reads_configured_command() {
        let config = GitConfig::parse(
            r#"
            [credential]
                helper = cache
                helper = manager
            "#,
        )
        .expect("config should parse");

        let helper = GitCredentialHelper::from_config(&config).expect("helper should exist");

        assert_eq!(helper.command, "manager");
    }
}
