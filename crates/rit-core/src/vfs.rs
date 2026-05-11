use crate::LazyMaterializationPolicy;

mod materialize;
mod platform;

pub use materialize::{VfsMaterializeRequest, VfsMaterializeResult, VfsMaterializeStatus};
pub use platform::{VfsPlatformBackend, VfsPlatformBackendPlan};

/// Requested VFS backend.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum VfsBackendPreference {
    /// Let rit choose the safest backend for the current platform and build.
    #[default]
    Auto,
    /// Use a plain materialized working tree fallback.
    FallbackMaterialized,
    /// Use a platform-specific virtual filesystem backend.
    Platform,
}

/// Current build/platform support for VFS.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VfsAvailability {
    /// VFS support is compiled in.
    Available,
    /// This binary was built without the `vfs` feature.
    BuildDisabled {
        /// Cargo feature needed for VFS support.
        feature: &'static str,
    },
}

impl VfsAvailability {
    /// Returns availability for the current binary.
    pub fn current() -> Self {
        if cfg!(feature = "vfs") {
            Self::Available
        } else {
            Self::BuildDisabled { feature: "vfs" }
        }
    }

    /// Returns a clear user-facing availability message.
    pub fn message(&self) -> &'static str {
        match self {
            Self::Available => "VFS support is available in this rit build",
            Self::BuildDisabled { feature } => match *feature {
                "vfs" => {
                    "This rit build does not include VFS support; rebuild with the `vfs` feature"
                }
                _ => "This rit build does not include the required VFS feature",
            },
        }
    }
}

/// Lazy materialization settings for a VFS plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VfsLazyMaterialization {
    /// Whether files may be materialized lazily.
    pub enabled: bool,
    /// Repository-relative paths included by the lazy workspace.
    pub include: Vec<String>,
    /// Whether missing content should come from partial clone.
    pub requires_partial_clone: bool,
}

impl From<&LazyMaterializationPolicy> for VfsLazyMaterialization {
    fn from(policy: &LazyMaterializationPolicy) -> Self {
        Self {
            enabled: policy.enabled,
            include: policy.include.clone(),
            requires_partial_clone: policy.requires_partial_clone,
        }
    }
}

/// Common VFS planning model shared by future backends.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VfsPlan {
    /// Workspace name when derived from a workspace profile.
    pub workspace: Option<String>,
    /// Requested backend preference.
    pub backend: VfsBackendPreference,
    /// Whether this binary can run VFS behavior.
    pub availability: VfsAvailability,
    /// Lazy materialization policy.
    pub lazy_materialization: VfsLazyMaterialization,
    /// Whether background prefetch is requested.
    pub background_prefetch: bool,
}

impl VfsPlan {
    /// Builds a disabled VFS plan.
    pub fn disabled() -> Self {
        Self {
            workspace: None,
            backend: VfsBackendPreference::FallbackMaterialized,
            availability: VfsAvailability::current(),
            lazy_materialization: VfsLazyMaterialization {
                enabled: false,
                include: Vec::new(),
                requires_partial_clone: false,
            },
            background_prefetch: false,
        }
    }

    /// Builds a VFS plan from a workspace lazy materialization policy.
    pub fn from_lazy_policy(
        policy: &LazyMaterializationPolicy,
        backend: VfsBackendPreference,
        background_prefetch: bool,
    ) -> Self {
        Self {
            workspace: Some(policy.workspace.clone()),
            backend,
            availability: VfsAvailability::current(),
            lazy_materialization: VfsLazyMaterialization::from(policy),
            background_prefetch,
        }
    }

    /// Returns true when the plan requests virtual behavior but the build lacks it.
    pub fn needs_unavailable_vfs(&self) -> bool {
        self.lazy_materialization.enabled
            && self.backend != VfsBackendPreference::FallbackMaterialized
            && self.availability != VfsAvailability::Available
    }
}

/// Backend that keeps files as ordinary materialized worktree files.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FallbackMaterializedBackend;

impl FallbackMaterializedBackend {
    /// Builds a fallback execution plan without touching the filesystem.
    pub fn plan(&self, vfs_plan: &VfsPlan) -> FallbackMaterializedPlan {
        let actions = if vfs_plan.lazy_materialization.include.is_empty() {
            vec![FallbackMaterializedAction::KeepFullWorktreeMaterialized]
        } else {
            vfs_plan
                .lazy_materialization
                .include
                .iter()
                .cloned()
                .map(|path| FallbackMaterializedAction::KeepPathMaterialized { path })
                .collect()
        };

        FallbackMaterializedPlan {
            workspace: vfs_plan.workspace.clone(),
            actions,
            partial_clone_required: vfs_plan.lazy_materialization.requires_partial_clone,
            background_prefetch_requested: vfs_plan.background_prefetch,
        }
    }
}

/// Fallback materialized backend plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FallbackMaterializedPlan {
    /// Workspace name, when known.
    pub workspace: Option<String>,
    /// Materialization actions to keep the worktree ordinary and explicit.
    pub actions: Vec<FallbackMaterializedAction>,
    /// Whether the source VFS plan expects partial clone for missing blobs.
    pub partial_clone_required: bool,
    /// Whether background prefetch was requested by the source VFS plan.
    pub background_prefetch_requested: bool,
}

/// One fallback materialization action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FallbackMaterializedAction {
    /// Keep the whole worktree as a normal materialized checkout.
    KeepFullWorktreeMaterialized,
    /// Keep one included path materialized as normal files.
    KeepPathMaterialized {
        /// Repository-relative path.
        path: String,
    },
}
