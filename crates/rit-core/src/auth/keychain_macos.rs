use super::super::{Credential, CredentialKind, CredentialRequest, SecretString};
use crate::{Result, RitError};
use std::process::Command;

pub fn read(target: &str, request: &CredentialRequest) -> Result<Option<Credential>> {
    let output = Command::new("security")
        .args(read_args(target, request))
        .output()
        .map_err(|source| RitError::transport_io("macOS Keychain", source))?;
    if !output.status.success() {
        if output.status.code() == Some(44) {
            return Ok(None);
        }
        return Err(RitError::invalid_input(
            "macOS Keychain credential lookup failed",
        ));
    }
    let secret = String::from_utf8(output.stdout)
        .map_err(|_| RitError::invalid_input("macOS Keychain returned non-UTF-8 secret"))?;
    Ok(Some(Credential {
        username: request.username.clone(),
        secret: SecretString::new(secret.trim_end_matches(['\r', '\n']).to_owned()),
        kind: if request.username.is_some() {
            CredentialKind::Password
        } else {
            CredentialKind::Token
        },
    }))
}

pub fn store(target: &str, request: &CredentialRequest, credential: &Credential) -> Result<()> {
    let status = Command::new("security")
        .args(store_args(target, request, credential))
        .status()
        .map_err(|source| RitError::transport_io("macOS Keychain", source))?;
    if !status.success() {
        return Err(RitError::invalid_input(
            "macOS Keychain credential store failed",
        ));
    }
    Ok(())
}

pub fn erase(target: &str, request: &CredentialRequest) -> Result<()> {
    let status = Command::new("security")
        .args(erase_args(target, request))
        .status()
        .map_err(|source| RitError::transport_io("macOS Keychain", source))?;
    if !status.success() && status.code() != Some(44) {
        return Err(RitError::invalid_input(
            "macOS Keychain credential erase failed",
        ));
    }
    Ok(())
}

fn read_args(target: &str, request: &CredentialRequest) -> Vec<String> {
    let mut args = vec![
        "find-generic-password".to_owned(),
        "-s".to_owned(),
        target.to_owned(),
        "-w".to_owned(),
    ];
    if let Some(username) = &request.username {
        args.push("-a".to_owned());
        args.push(username.clone());
    }
    args
}

fn store_args(target: &str, request: &CredentialRequest, credential: &Credential) -> Vec<String> {
    vec![
        "add-generic-password".to_owned(),
        "-U".to_owned(),
        "-s".to_owned(),
        target.to_owned(),
        "-a".to_owned(),
        credential
            .username
            .as_ref()
            .or(request.username.as_ref())
            .cloned()
            .unwrap_or_else(|| "rit-token".to_owned()),
        "-w".to_owned(),
        credential.secret.expose_secret().to_owned(),
    ]
}

fn erase_args(target: &str, request: &CredentialRequest) -> Vec<String> {
    let mut args = vec![
        "delete-generic-password".to_owned(),
        "-s".to_owned(),
        target.to_owned(),
    ];
    if let Some(username) = &request.username {
        args.push("-a".to_owned());
        args.push(username.clone());
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_security_arguments_include_target_and_account() {
        let mut request = CredentialRequest::new("https", "example.test");
        request.username = Some("alice".to_owned());

        assert_eq!(
            read_args("rit:https://example.test", &request),
            vec![
                "find-generic-password",
                "-s",
                "rit:https://example.test",
                "-w",
                "-a",
                "alice"
            ]
        );
        assert_eq!(
            erase_args("rit:https://example.test", &request),
            vec![
                "delete-generic-password",
                "-s",
                "rit:https://example.test",
                "-a",
                "alice"
            ]
        );
    }
}
