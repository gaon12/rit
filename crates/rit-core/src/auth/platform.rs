use std::env;
use std::path::PathBuf;

/// SSH agent connection settings.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SshAgentConfig {
    /// Path from `SSH_AUTH_SOCK`, when available.
    pub socket: Option<PathBuf>,
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
