use super::{
    Credential, CredentialKind, CredentialProvider, CredentialRequest, GitCredentialHelper,
    GitCredentialMessage, SecretString,
};
use crate::{GitConfig, Result, RitError};
use std::io::Write;
use std::process::{Command, Stdio};

/// Operation argument appended to a Git credential helper command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GitCredentialHelperOperation {
    /// Ask a helper to return matching credential attributes.
    Get,
    /// Ask a helper to persist credential attributes.
    Store,
    /// Ask a helper to remove matching credential attributes.
    Erase,
}

impl GitCredentialHelperOperation {
    /// Returns the operation spelling used by Git credential helpers.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "get",
            Self::Store => "store",
            Self::Erase => "erase",
        }
    }
}

/// Executes a single credential helper operation.
pub trait GitCredentialHelperExecutor {
    /// Runs `helper` with `operation`, writing `input` to stdin and returning
    /// parsed stdout attributes.
    fn run(
        &self,
        helper: &GitCredentialHelper,
        operation: GitCredentialHelperOperation,
        input: &GitCredentialMessage,
    ) -> Result<GitCredentialMessage>;
}

/// Process-backed credential helper executor.
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcessGitCredentialHelperExecutor;

impl GitCredentialHelperExecutor for ProcessGitCredentialHelperExecutor {
    fn run(
        &self,
        helper: &GitCredentialHelper,
        operation: GitCredentialHelperOperation,
        input: &GitCredentialMessage,
    ) -> Result<GitCredentialMessage> {
        let command_line = helper.command_line_for_operation(operation.as_str());
        let mut command = shell_command(&command_line);
        command.stdin(Stdio::piped());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        let mut child = command
            .spawn()
            .map_err(|source| RitError::transport_io("credential helper", source))?;

        if let Some(stdin) = child.stdin.as_mut() {
            stdin
                .write_all(input.to_protocol_text().as_bytes())
                .map_err(|source| RitError::transport_io("credential helper stdin", source))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|source| RitError::transport_io("credential helper", source))?;

        if !output.status.success() {
            return Err(RitError::invalid_input(format!(
                "credential helper failed with status {}",
                output.status
            )));
        }

        Ok(GitCredentialMessage::parse_protocol_text(
            &String::from_utf8_lossy(&output.stdout),
        ))
    }
}

/// Credential provider backed by a configured Git credential helper chain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitCredentialHelperProvider {
    helpers: Vec<GitCredentialHelper>,
}

impl GitCredentialHelperProvider {
    /// Builds a provider from an ordered helper chain.
    pub fn new(helpers: Vec<GitCredentialHelper>) -> Self {
        Self { helpers }
    }

    /// Reads helper chain configuration from Git config.
    pub fn from_config(config: &GitConfig) -> Self {
        Self::new(GitCredentialHelper::chain_from_config(config))
    }

    /// Returns the configured helper chain.
    pub fn helpers(&self) -> &[GitCredentialHelper] {
        &self.helpers
    }

    /// Looks up a credential using an injected helper executor.
    pub fn credential_with_executor(
        &self,
        request: &CredentialRequest,
        executor: &impl GitCredentialHelperExecutor,
    ) -> Result<Option<Credential>> {
        let mut message = GitCredentialMessage::from_request(request);

        for helper in &self.helpers {
            let response = executor.run(helper, GitCredentialHelperOperation::Get, &message)?;
            merge_helper_response(&mut message, response);

            if let Some(credential) = message_to_credential(&message) {
                return Ok(Some(credential));
            }
            if message.quit {
                break;
            }
        }

        Ok(None)
    }

    /// Stores a credential in every configured helper that supports storage.
    pub fn store_with_executor(
        &self,
        request: &CredentialRequest,
        credential: &Credential,
        executor: &impl GitCredentialHelperExecutor,
    ) -> Result<()> {
        let mut message = GitCredentialMessage::from_request(request);
        if credential.username.is_some() {
            message.username = credential.username.clone();
        }
        message.password = Some(credential.secret.clone());

        for helper in &self.helpers {
            executor.run(helper, GitCredentialHelperOperation::Store, &message)?;
        }

        Ok(())
    }

    /// Erases matching credentials from every configured helper.
    pub fn erase_with_executor(
        &self,
        request: &CredentialRequest,
        executor: &impl GitCredentialHelperExecutor,
    ) -> Result<()> {
        let message = GitCredentialMessage::from_request(request);

        for helper in &self.helpers {
            executor.run(helper, GitCredentialHelperOperation::Erase, &message)?;
        }

        Ok(())
    }
}

impl CredentialProvider for GitCredentialHelperProvider {
    fn credential(&self, request: &CredentialRequest) -> Result<Option<Credential>> {
        self.credential_with_executor(request, &ProcessGitCredentialHelperExecutor)
    }
}

fn merge_helper_response(message: &mut GitCredentialMessage, response: GitCredentialMessage) {
    if let Some(protocol) = response.protocol {
        message.protocol = Some(protocol);
    }
    if let Some(host) = response.host {
        message.host = Some(host);
    }
    if let Some(path) = response.path {
        message.path = Some(path);
    }
    if let Some(username) = response.username {
        message.username = Some(username);
    }
    if let Some(password) = response.password {
        message.password = Some(password);
    }
    if response.quit {
        message.quit = true;
    }
}

fn message_to_credential(message: &GitCredentialMessage) -> Option<Credential> {
    let username = message.username.clone()?;
    let password = message.password.as_ref()?;
    Some(Credential {
        username: Some(username),
        secret: SecretString::new(password.expose_secret()),
        kind: CredentialKind::Password,
    })
}

#[cfg(windows)]
fn shell_command(command_line: &str) -> Command {
    let mut command = Command::new("cmd");
    command.arg("/C").arg(command_line);
    command
}

#[cfg(not(windows))]
fn shell_command(command_line: &str) -> Command {
    let mut command = Command::new("sh");
    command.arg("-c").arg(command_line);
    command
}
