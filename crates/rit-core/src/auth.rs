mod credential;
mod environment;
mod git_credential;
mod git_credential_process;
#[cfg(test)]
mod git_credential_process_tests;
mod interaction;
mod keychain;
mod platform;
mod ssh_agent;
#[cfg(test)]
mod ssh_agent_tests;

pub use credential::{
    Credential, CredentialKind, CredentialProvider, CredentialRequest, SecretString,
};
pub use environment::{DEFAULT_TOKEN_ENV_VARS, EnvironmentToken, EnvironmentTokenProvider};
pub use git_credential::{GitCredentialHelper, GitCredentialMessage};
pub use git_credential_process::{
    GitCredentialHelperExecutor, GitCredentialHelperOperation, GitCredentialHelperProvider,
    ProcessGitCredentialHelperExecutor,
};
pub use interaction::AuthInteractionPolicy;
pub use keychain::SystemKeychainProvider;
pub use platform::{KeychainProviderKind, SshAgentConfig, SystemKeychainConfig};
pub use ssh_agent::{SshAgentClient, SshAgentIdentity, SshAgentSignFlags, SshAgentSignature};
