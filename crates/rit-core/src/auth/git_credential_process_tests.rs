use super::{
    Credential, CredentialProvider, CredentialRequest, GitCredentialHelper,
    GitCredentialHelperExecutor, GitCredentialHelperOperation, GitCredentialHelperProvider,
    GitCredentialMessage, SecretString,
};
use crate::Result;
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
