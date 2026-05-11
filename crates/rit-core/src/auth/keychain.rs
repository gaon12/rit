use super::{Credential, CredentialProvider, CredentialRequest, SystemKeychainConfig};
use crate::Result;

#[cfg(test)]
#[path = "keychain_tests.rs"]
mod tests;
#[cfg(windows)]
#[path = "keychain_windows.rs"]
mod windows_keychain;

/// Credential provider backed by the platform's default keychain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemKeychainProvider {
    config: SystemKeychainConfig,
}

impl SystemKeychainProvider {
    /// Creates a provider from explicit keychain selection.
    pub fn new(config: SystemKeychainConfig) -> Self {
        Self { config }
    }

    /// Selects the default provider for the current platform.
    pub fn current_platform() -> Self {
        Self::new(SystemKeychainConfig::current_platform())
    }

    /// Returns the keychain selection used by this provider.
    pub fn config(&self) -> &SystemKeychainConfig {
        &self.config
    }

    /// Stores a credential in the selected system keychain.
    pub fn store(&self, request: &CredentialRequest, credential: &Credential) -> Result<()> {
        platform_store(request, credential)
    }

    /// Removes a credential matching `request` from the selected system keychain.
    pub fn erase(&self, request: &CredentialRequest) -> Result<()> {
        platform_erase(request)
    }
}

impl CredentialProvider for SystemKeychainProvider {
    fn credential(&self, request: &CredentialRequest) -> Result<Option<Credential>> {
        platform_read(request)
    }
}

fn keychain_target(request: &CredentialRequest) -> String {
    let mut target = format!("rit:{}://{}", request.protocol, request.host);
    if let Some(path) = &request.path {
        target.push('/');
        target.push_str(path);
    }
    target
}

#[cfg(windows)]
fn platform_read(request: &CredentialRequest) -> Result<Option<Credential>> {
    windows_keychain::read(&keychain_target(request))
}

#[cfg(windows)]
fn platform_store(request: &CredentialRequest, credential: &Credential) -> Result<()> {
    windows_keychain::store(&keychain_target(request), credential)
}

#[cfg(windows)]
fn platform_erase(request: &CredentialRequest) -> Result<()> {
    windows_keychain::erase(&keychain_target(request))
}

#[cfg(not(windows))]
fn platform_read(_request: &CredentialRequest) -> Result<Option<Credential>> {
    Err(unsupported_keychain_error())
}

#[cfg(not(windows))]
fn platform_store(_request: &CredentialRequest, _credential: &Credential) -> Result<()> {
    Err(unsupported_keychain_error())
}

#[cfg(not(windows))]
fn platform_erase(_request: &CredentialRequest) -> Result<()> {
    Err(unsupported_keychain_error())
}

#[cfg(not(windows))]
fn unsupported_keychain_error() -> crate::RitError {
    crate::RitError::invalid_input(
        "system keychain read/write is not implemented for this platform yet",
    )
}
