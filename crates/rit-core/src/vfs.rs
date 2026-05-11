use crate::LazyMaterializationPolicy;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_plan_uses_fallback_materialized_backend() {
        let plan = VfsPlan::disabled();

        assert_eq!(plan.backend, VfsBackendPreference::FallbackMaterialized);
        assert!(!plan.lazy_materialization.enabled);
        assert!(!plan.needs_unavailable_vfs());
    }

    #[test]
    fn plan_from_lazy_policy_keeps_workspace_paths() {
        let policy = LazyMaterializationPolicy {
            workspace: "mobile".to_owned(),
            enabled: true,
            include: vec!["apps/mobile".to_owned(), "packages/ui".to_owned()],
            requires_partial_clone: true,
        };

        let plan = VfsPlan::from_lazy_policy(&policy, VfsBackendPreference::Auto, true);

        assert_eq!(plan.workspace.as_deref(), Some("mobile"));
        assert_eq!(plan.lazy_materialization.include, policy.include);
        assert!(plan.lazy_materialization.requires_partial_clone);
        assert!(plan.background_prefetch);
    }

    #[test]
    fn availability_message_is_clear() {
        let availability = VfsAvailability::current();

        assert!(availability.message().contains("VFS"));
    }
}
