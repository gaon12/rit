use std::env;

/// Policy controlling whether auth may prompt interactively.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthInteractionPolicy {
    /// Whether interactive credential prompts are allowed.
    pub allow_prompts: bool,
    /// Human-readable reason prompts are disabled.
    pub disabled_reason: Option<String>,
}

impl AuthInteractionPolicy {
    /// Reads CI/non-interactive settings from the current environment.
    pub fn from_env() -> Self {
        let pairs = [
            ("CI", env::var("CI").ok()),
            ("GITHUB_ACTIONS", env::var("GITHUB_ACTIONS").ok()),
            ("GIT_TERMINAL_PROMPT", env::var("GIT_TERMINAL_PROMPT").ok()),
            ("RIT_NONINTERACTIVE", env::var("RIT_NONINTERACTIVE").ok()),
        ];
        Self::from_pairs(pairs)
    }

    /// Builds a policy from explicit key/value pairs.
    pub fn from_pairs<const N: usize>(pairs: [(&str, Option<String>); N]) -> Self {
        for (key, value) in pairs {
            let Some(value) = value else {
                continue;
            };
            if key == "GIT_TERMINAL_PROMPT" && value == "0" {
                return Self::disabled("GIT_TERMINAL_PROMPT=0");
            }
            if matches!(key, "CI" | "GITHUB_ACTIONS" | "RIT_NONINTERACTIVE")
                && is_truthy_env_value(&value)
            {
                return Self::disabled(format!("{key}={value}"));
            }
        }
        Self {
            allow_prompts: true,
            disabled_reason: None,
        }
    }

    fn disabled(reason: impl Into<String>) -> Self {
        Self {
            allow_prompts: false,
            disabled_reason: Some(reason.into()),
        }
    }
}

fn is_truthy_env_value(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_interaction_policy_disables_prompts_in_ci() {
        let policy = AuthInteractionPolicy::from_pairs([
            ("CI", Some("true".to_owned())),
            ("GIT_TERMINAL_PROMPT", None),
        ]);

        assert!(!policy.allow_prompts);
        assert_eq!(policy.disabled_reason.as_deref(), Some("CI=true"));
    }

    #[test]
    fn auth_interaction_policy_honors_git_terminal_prompt_zero() {
        let policy =
            AuthInteractionPolicy::from_pairs([("GIT_TERMINAL_PROMPT", Some("0".to_owned()))]);

        assert!(!policy.allow_prompts);
        assert_eq!(
            policy.disabled_reason.as_deref(),
            Some("GIT_TERMINAL_PROMPT=0")
        );
    }
}
