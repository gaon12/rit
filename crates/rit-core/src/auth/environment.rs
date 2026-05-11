use super::{Credential, CredentialProvider, CredentialRequest};
use crate::Result;
use std::env;

/// Environment variables checked by the default token provider.
pub const DEFAULT_TOKEN_ENV_VARS: &[&str] = &[
    "RIT_TOKEN",
    "GIT_TOKEN",
    "GITHUB_TOKEN",
    "GITLAB_TOKEN",
    "HF_TOKEN",
];

/// Token provider backed by environment variables.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EnvironmentTokenProvider {
    tokens: Vec<EnvironmentToken>,
}

/// One token discovered from an environment variable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentToken {
    /// Environment variable name.
    pub variable: String,
    /// Credential built from the variable value.
    pub credential: Credential,
}

impl EnvironmentTokenProvider {
    /// Reads the default environment token variables.
    pub fn from_env() -> Self {
        Self::from_env_vars(DEFAULT_TOKEN_ENV_VARS)
    }

    /// Reads a caller-provided list of environment variable names.
    pub fn from_env_vars(variables: &[&str]) -> Self {
        let tokens = variables
            .iter()
            .filter_map(|variable| {
                let value = env::var(variable).ok()?;
                if value.is_empty() {
                    return None;
                }
                Some(EnvironmentToken {
                    variable: (*variable).to_owned(),
                    credential: Credential::token(value),
                })
            })
            .collect();
        Self { tokens }
    }

    /// Builds a provider from explicit variable/token pairs.
    pub fn from_tokens(tokens: Vec<(impl Into<String>, impl Into<String>)>) -> Self {
        Self {
            tokens: tokens
                .into_iter()
                .map(|(variable, token)| EnvironmentToken {
                    variable: variable.into(),
                    credential: Credential::token(token),
                })
                .collect(),
        }
    }

    /// Returns true when at least one usable token is configured.
    pub fn has_tokens(&self) -> bool {
        !self.tokens.is_empty()
    }
}

impl CredentialProvider for EnvironmentTokenProvider {
    fn credential(&self, request: &CredentialRequest) -> Result<Option<Credential>> {
        let Some(token) = self.tokens.first() else {
            return Ok(None);
        };
        let mut credential = token.credential.clone();
        if credential.username.is_none() {
            credential.username = request.username.clone();
        }
        Ok(Some(credential))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_token_provider_returns_redacted_token() {
        let provider =
            EnvironmentTokenProvider::from_tokens(vec![("RIT_TOKEN", "super-secret-token")]);
        let mut request = CredentialRequest::new("https", "example.test");
        request.username = Some("alice".to_owned());

        let credential = provider
            .credential(&request)
            .expect("provider should not fail")
            .expect("token should exist");

        assert!(provider.has_tokens());
        assert_eq!(credential.username.as_deref(), Some("alice"));
        assert_eq!(credential.secret.expose_secret(), "super-secret-token");
        assert!(!format!("{provider:?}").contains("super-secret-token"));
    }

    #[test]
    fn empty_environment_token_provider_returns_none() {
        let provider = EnvironmentTokenProvider::default();
        let request = CredentialRequest::new("https", "example.test");

        assert_eq!(
            provider
                .credential(&request)
                .expect("provider should not fail"),
            None
        );
    }
}
