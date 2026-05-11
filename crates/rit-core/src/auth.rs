use crate::{GitConfig, Result};
use std::env;
use std::fmt::{Debug, Display, Formatter};
use std::path::PathBuf;

/// Environment variables checked by the default token provider.
pub const DEFAULT_TOKEN_ENV_VARS: &[&str] = &[
    "RIT_TOKEN",
    "GIT_TOKEN",
    "GITHUB_TOKEN",
    "GITLAB_TOKEN",
    "HF_TOKEN",
];

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

/// Token provider backed by environment variables.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EnvironmentTokenProvider {
    tokens: Vec<EnvironmentToken>,
}

/// One token discovered from an environment variable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentToken {
    /// Environment variable name.
    pub variable: String,
    /// Credential built from the variable value.
    pub credential: Credential,
}

impl EnvironmentTokenProvider {
    /// Reads the default environment token variables.
    pub fn from_env() -> Self {
        Self::from_env_vars(DEFAULT_TOKEN_ENV_VARS)
    }

    /// Reads a caller-provided list of environment variable names.
    pub fn from_env_vars(variables: &[&str]) -> Self {
        let tokens = variables
            .iter()
            .filter_map(|variable| {
                let value = env::var(variable).ok()?;
                if value.is_empty() {
                    return None;
                }
                Some(EnvironmentToken {
                    variable: (*variable).to_owned(),
                    credential: Credential::token(value),
                })
            })
            .collect();
        Self { tokens }
    }

    /// Builds a provider from explicit variable/token pairs.
    pub fn from_tokens(tokens: Vec<(impl Into<String>, impl Into<String>)>) -> Self {
        Self {
            tokens: tokens
                .into_iter()
                .map(|(variable, token)| EnvironmentToken {
                    variable: variable.into(),
                    credential: Credential::token(token),
                })
                .collect(),
        }
    }

    /// Returns true when at least one usable token is configured.
    pub fn has_tokens(&self) -> bool {
        !self.tokens.is_empty()
    }
}

impl CredentialProvider for EnvironmentTokenProvider {
    fn credential(&self, request: &CredentialRequest) -> Result<Option<Credential>> {
        let Some(token) = self.tokens.first() else {
            return Ok(None);
        };
        let mut credential = token.credential.clone();
        if credential.username.is_none() {
            credential.username = request.username.clone();
        }
        Ok(Some(credential))
    }
}

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

/// SSH agent connection settings.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SshAgentConfig {
    /// Path from `SSH_AUTH_SOCK`, when available.
    pub socket: Option<PathBuf>,
}

/// Supported OS keychain adapter families.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeychainProviderKind {
    /// Windows Credential Manager.
    WindowsCredentialManager,
    /// macOS Keychain.
    MacosKeychain,
    /// Freedesktop Secret Service/libsecret.
    Libsecret,
}

/// OS keychain adapter selection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemKeychainConfig {
    /// Selected adapter for the current platform, if one is known.
    pub provider: Option<KeychainProviderKind>,
}

/// Policy controlling whether auth may prompt interactively.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthInteractionPolicy {
    /// Whether interactive credential prompts are allowed.
    pub allow_prompts: bool,
    /// Human-readable reason prompts are disabled.
    pub disabled_reason: Option<String>,
}

impl AuthInteractionPolicy {
    /// Reads CI/non-interactive settings from the current environment.
    pub fn from_env() -> Self {
        let pairs = [
            ("CI", env::var("CI").ok()),
            ("GITHUB_ACTIONS", env::var("GITHUB_ACTIONS").ok()),
            ("GIT_TERMINAL_PROMPT", env::var("GIT_TERMINAL_PROMPT").ok()),
            ("RIT_NONINTERACTIVE", env::var("RIT_NONINTERACTIVE").ok()),
        ];
        Self::from_pairs(pairs)
    }

    /// Builds a policy from explicit key/value pairs.
    pub fn from_pairs<const N: usize>(pairs: [(&str, Option<String>); N]) -> Self {
        for (key, value) in pairs {
            let Some(value) = value else {
                continue;
            };
            if key == "GIT_TERMINAL_PROMPT" && value == "0" {
                return Self::disabled("GIT_TERMINAL_PROMPT=0");
            }
            if matches!(key, "CI" | "GITHUB_ACTIONS" | "RIT_NONINTERACTIVE")
                && is_truthy_env_value(&value)
            {
                return Self::disabled(format!("{key}={value}"));
            }
        }
        Self {
            allow_prompts: true,
            disabled_reason: None,
        }
    }

    fn disabled(reason: impl Into<String>) -> Self {
        Self {
            allow_prompts: false,
            disabled_reason: Some(reason.into()),
        }
    }
}

impl SystemKeychainConfig {
    /// Selects the default adapter for the current platform.
    pub fn current_platform() -> Self {
        Self {
            provider: default_keychain_provider(),
        }
    }

    /// Returns true when rit has a known adapter family for this platform.
    pub fn is_available(&self) -> bool {
        self.provider.is_some()
    }
}

impl SshAgentConfig {
    /// Reads SSH agent settings from the current environment.
    pub fn from_env() -> Self {
        Self {
            socket: env::var_os("SSH_AUTH_SOCK").map(PathBuf::from),
        }
    }

    /// Creates explicit SSH agent settings.
    pub fn new(socket: Option<impl Into<PathBuf>>) -> Self {
        Self {
            socket: socket.map(Into::into),
        }
    }

    /// Returns true when an agent socket is configured.
    pub fn is_available(&self) -> bool {
        self.socket.is_some()
    }
}

#[cfg(windows)]
fn default_keychain_provider() -> Option<KeychainProviderKind> {
    Some(KeychainProviderKind::WindowsCredentialManager)
}

#[cfg(target_os = "macos")]
fn default_keychain_provider() -> Option<KeychainProviderKind> {
    Some(KeychainProviderKind::MacosKeychain)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn default_keychain_provider() -> Option<KeychainProviderKind> {
    Some(KeychainProviderKind::Libsecret)
}

#[cfg(not(any(windows, unix)))]
fn default_keychain_provider() -> Option<KeychainProviderKind> {
    None
}

fn is_truthy_env_value(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
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

    #[test]
    fn environment_token_provider_returns_redacted_token() {
        let provider =
            EnvironmentTokenProvider::from_tokens(vec![("RIT_TOKEN", "super-secret-token")]);
        let mut request = CredentialRequest::new("https", "example.test");
        request.username = Some("alice".to_owned());

        let credential = provider
            .credential(&request)
            .expect("provider should not fail")
            .expect("token should exist");

        assert!(provider.has_tokens());
        assert_eq!(credential.username.as_deref(), Some("alice"));
        assert_eq!(credential.secret.expose_secret(), "super-secret-token");
        assert!(!format!("{provider:?}").contains("super-secret-token"));
    }

    #[test]
    fn empty_environment_token_provider_returns_none() {
        let provider = EnvironmentTokenProvider::default();
        let request = CredentialRequest::new("https", "example.test");

        assert_eq!(
            provider
                .credential(&request)
                .expect("provider should not fail"),
            None
        );
    }

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

    #[test]
    fn ssh_agent_config_reports_socket_availability() {
        let config = SshAgentConfig::new(Some("/tmp/agent.sock"));

        assert!(config.is_available());
        assert_eq!(config.socket, Some(PathBuf::from("/tmp/agent.sock")));

        let missing = SshAgentConfig::new(None::<PathBuf>);
        assert!(!missing.is_available());
    }

    #[test]
    fn system_keychain_config_selects_known_platform_adapter() {
        let config = SystemKeychainConfig::current_platform();

        if cfg!(any(windows, unix)) {
            assert!(config.is_available());
        } else {
            assert!(!config.is_available());
        }
    }

    #[test]
    fn auth_interaction_policy_disables_prompts_in_ci() {
        let policy = AuthInteractionPolicy::from_pairs([
            ("CI", Some("true".to_owned())),
            ("GIT_TERMINAL_PROMPT", None),
        ]);

        assert!(!policy.allow_prompts);
        assert_eq!(policy.disabled_reason.as_deref(), Some("CI=true"));
    }

    #[test]
    fn auth_interaction_policy_honors_git_terminal_prompt_zero() {
        let policy =
            AuthInteractionPolicy::from_pairs([("GIT_TERMINAL_PROMPT", Some("0".to_owned()))]);

        assert!(!policy.allow_prompts);
        assert_eq!(
            policy.disabled_reason.as_deref(),
            Some("GIT_TERMINAL_PROMPT=0")
        );
    }
}
