mod credential;
mod environment;
mod git_credential;
mod git_credential_process;
mod interaction;
mod platform;

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
pub use platform::{KeychainProviderKind, SshAgentConfig, SystemKeychainConfig};
