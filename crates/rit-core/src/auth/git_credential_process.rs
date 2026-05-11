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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[derive(Default)]
    struct FakeExecutor {
        responses: RefCell<VecDeque<GitCredentialMessage>>,
        calls: RefCell<Vec<(String, GitCredentialHelperOperation, GitCredentialMessage)>>,
    }

    impl FakeExecutor {
        fn with_responses(responses: Vec<GitCredentialMessage>) -> Self {
            Self {
                responses: RefCell::new(responses.into()),
                calls: RefCell::new(Vec::new()),
            }
        }
    }

    impl GitCredentialHelperExecutor for FakeExecutor {
        fn run(
            &self,
            helper: &GitCredentialHelper,
            operation: GitCredentialHelperOperation,
            input: &GitCredentialMessage,
        ) -> Result<GitCredentialMessage> {
            self.calls
                .borrow_mut()
                .push((helper.command.clone(), operation, input.clone()));
            Ok(self.responses.borrow_mut().pop_front().unwrap_or_default())
        }
    }

    #[test]
    fn provider_merges_helper_responses_until_username_and_password_exist() {
        let provider = GitCredentialHelperProvider::new(vec![
            GitCredentialHelper {
                command: "first".to_owned(),
            },
            GitCredentialHelper {
                command: "second".to_owned(),
            },
            GitCredentialHelper {
                command: "third".to_owned(),
            },
        ]);
        let executor = FakeExecutor::with_responses(vec![
            GitCredentialMessage {
                username: Some("alice".to_owned()),
                ..Default::default()
            },
            GitCredentialMessage {
                password: Some(SecretString::new("secret")),
                ..Default::default()
            },
        ]);

        let credential = provider
            .credential_with_executor(&CredentialRequest::new("https", "example.test"), &executor)
            .expect("lookup should succeed")
            .expect("credential should be returned");

        assert_eq!(credential.username.as_deref(), Some("alice"));
        assert_eq!(credential.secret.expose_secret(), "secret");
        assert_eq!(executor.calls.borrow().len(), 2);
    }

    #[test]
    fn provider_honors_quit_without_later_helpers() {
        let provider = GitCredentialHelperProvider::new(vec![
            GitCredentialHelper {
                command: "first".to_owned(),
            },
            GitCredentialHelper {
                command: "second".to_owned(),
            },
        ]);
        let executor = FakeExecutor::with_responses(vec![GitCredentialMessage {
            quit: true,
            ..Default::default()
        }]);

        let credential = provider
            .credential_with_executor(&CredentialRequest::new("https", "example.test"), &executor)
            .expect("lookup should succeed");

        assert_eq!(credential, None);
        assert_eq!(executor.calls.borrow().len(), 1);
    }

    #[test]
    fn provider_stores_and_erases_with_all_helpers() {
        let provider = GitCredentialHelperProvider::new(vec![
            GitCredentialHelper {
                command: "first".to_owned(),
            },
            GitCredentialHelper {
                command: "second".to_owned(),
            },
        ]);
        let executor = FakeExecutor::default();
        let request = CredentialRequest::new("https", "example.test");

        provider
            .store_with_executor(
                &request,
                &Credential::password("alice", "secret"),
                &executor,
            )
            .expect("store should succeed");
        provider
            .erase_with_executor(&request, &executor)
            .expect("erase should succeed");

        let calls = executor.calls.borrow();
        assert_eq!(calls.len(), 4);
        assert_eq!(calls[0].1, GitCredentialHelperOperation::Store);
        assert_eq!(calls[1].1, GitCredentialHelperOperation::Store);
        assert_eq!(calls[2].1, GitCredentialHelperOperation::Erase);
        assert_eq!(calls[3].1, GitCredentialHelperOperation::Erase);
    }

    #[test]
    fn process_executor_reads_helper_stdout() {
        let helper_path = write_test_helper("process_executor_reads_helper_stdout");
        let provider = GitCredentialHelperProvider::new(vec![GitCredentialHelper {
            command: helper_path.to_string_lossy().into_owned(),
        }]);

        let credential = provider
            .credential(&CredentialRequest::new("https", "example.test"))
            .expect("process helper should run")
            .expect("helper should return a credential");

        assert_eq!(credential.username.as_deref(), Some("alice"));
        assert_eq!(credential.secret.expose_secret(), "secret");

        let _ = fs::remove_dir_all(helper_path.parent().expect("helper has parent"));
    }

    fn write_test_helper(name: &str) -> std::path::PathBuf {
        let directory = unique_temp_dir(name);
        fs::create_dir_all(&directory).expect("temp directory should be created");
        let helper_path = directory.join(helper_file_name());
        fs::write(&helper_path, helper_script()).expect("helper script should be written");
        make_executable(&helper_path);
        helper_path
    }

    fn unique_temp_dir(name: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("rit-credential-{name}-{unique}"))
    }

    #[cfg(windows)]
    fn helper_file_name() -> &'static str {
        "helper.cmd"
    }

    #[cfg(not(windows))]
    fn helper_file_name() -> &'static str {
        "helper"
    }

    #[cfg(windows)]
    fn helper_script() -> &'static str {
        "@echo off\r\nif \"%1\"==\"get\" (\r\n  echo username=alice\r\n  echo password=secret\r\n  echo.\r\n)\r\n"
    }

    #[cfg(not(windows))]
    fn helper_script() -> &'static str {
        "#!/bin/sh\nif [ \"$1\" = get ]; then\n  printf 'username=alice\\npassword=secret\\n\\n'\nfi\n"
    }

    #[cfg(windows)]
    fn make_executable(_path: &std::path::Path) {}

    #[cfg(not(windows))]
    fn make_executable(path: &std::path::Path) {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(path)
            .expect("helper metadata should be readable")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("helper should be executable");
    }
}
