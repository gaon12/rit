use super::{Credential, CredentialRequest, SecretString};
use crate::GitConfig;
use std::path::Path;

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
    /// Whether helpers should stop without consulting later providers.
    pub quit: bool,
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
        Self::chain_from_config(config).pop()
    }

    /// Reads configured `credential.helper` values as an ordered helper chain.
    ///
    /// An empty helper value clears previously configured helpers, matching
    /// Git's documented reset behavior for this multi-valued setting.
    pub fn chain_from_config(config: &GitConfig) -> Vec<Self> {
        let mut helpers = Vec::new();
        for helper in config.values("credential", "helper") {
            if helper.is_empty() {
                helpers.clear();
                continue;
            }
            helpers.push(Self {
                command: helper.to_owned(),
            });
        }
        helpers
    }

    /// Builds the shell command line Git would use for a helper operation,
    /// without routing through the `git` executable for named helpers.
    pub fn command_line_for_operation(&self, operation: &str) -> String {
        let command = self.command.trim();
        let command = if let Some(shell_snippet) = command.strip_prefix('!') {
            shell_snippet.to_owned()
        } else if helper_starts_with_absolute_path(command) {
            command.to_owned()
        } else {
            format!("git-credential-{command}")
        };
        format!("{command} {operation}")
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
            quit: false,
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
            quit: false,
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
        if self.quit {
            push_protocol_field(&mut output, "quit", Some("true"));
        }
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
                "quit" => message.quit = matches!(value, "true" | "1"),
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

fn helper_starts_with_absolute_path(command: &str) -> bool {
    let first_word = command.split_whitespace().next().unwrap_or_default();
    Path::new(first_word).is_absolute()
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
        assert!(!response.quit);
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

    #[test]
    fn git_credential_helper_reads_ordered_chain_with_reset() {
        let config = GitConfig::parse(
            r#"
            [credential]
                helper = cache
                helper =
                helper = manager
                helper = "store --file ~/.git-credentials"
            "#,
        )
        .expect("config should parse");

        let helpers = GitCredentialHelper::chain_from_config(&config);

        assert_eq!(
            helpers,
            vec![
                GitCredentialHelper {
                    command: "manager".to_owned(),
                },
                GitCredentialHelper {
                    command: "store --file ~/.git-credentials".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn git_credential_helper_builds_operation_command_lines() {
        assert_eq!(
            GitCredentialHelper {
                command: "cache --timeout=60".to_owned(),
            }
            .command_line_for_operation("get"),
            "git-credential-cache --timeout=60 get"
        );

        assert_eq!(
            GitCredentialHelper {
                command: "!f() { echo username=alice; }; f".to_owned(),
            }
            .command_line_for_operation("get"),
            "f() { echo username=alice; }; f get"
        );
    }

    #[test]
    fn git_credential_protocol_parses_quit() {
        let response = GitCredentialMessage::parse_protocol_text("quit=true\n\n");

        assert!(response.quit);
    }
}
