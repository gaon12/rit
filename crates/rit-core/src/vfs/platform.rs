use super::VfsAvailability;

/// Planned platform VFS backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VfsPlatformBackend {
    /// Windows Projected File System style backend.
    WindowsProjectedFileSystem,
    /// macOS FUSE-style backend.
    MacFuse,
    /// Linux FUSE-style backend.
    LinuxFuse,
}

impl VfsPlatformBackend {
    /// Returns the backend planned for the current operating system.
    pub fn current_platform() -> Self {
        if cfg!(windows) {
            Self::WindowsProjectedFileSystem
        } else if cfg!(target_os = "macos") {
            Self::MacFuse
        } else {
            Self::LinuxFuse
        }
    }

    /// Returns a concise backend name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::WindowsProjectedFileSystem => "windows-projected-file-system",
            Self::MacFuse => "macos-fuse",
            Self::LinuxFuse => "linux-fuse",
        }
    }
}

/// Platform backend planning result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VfsPlatformBackendPlan {
    /// Planned backend for this platform when VFS is available.
    pub backend: Option<VfsPlatformBackend>,
    /// Build-level VFS availability.
    pub availability: VfsAvailability,
    /// Human-readable plan message.
    pub message: String,
}

impl VfsPlatformBackendPlan {
    /// Builds a platform backend plan for the current binary and OS.
    pub fn current() -> Self {
        let availability = VfsAvailability::current();
        match availability {
            VfsAvailability::Available => {
                let backend = VfsPlatformBackend::current_platform();
                Self {
                    backend: Some(backend),
                    availability,
                    message: format!("planned platform VFS backend: {}", backend.name()),
                }
            }
            VfsAvailability::BuildDisabled { .. } => Self {
                backend: None,
                message: availability.message().to_owned(),
                availability,
            },
        }
    }
}
