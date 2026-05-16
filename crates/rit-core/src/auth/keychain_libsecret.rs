use super::super::{Credential, CredentialKind, CredentialRequest, SecretString};
use crate::{Result, RitError};
use std::io::Write;
use std::process::{Command, Stdio};

pub fn read(target: &str, request: &CredentialRequest) -> Result<Option<Credential>> {
    let output = Command::new("secret-tool")
        .args(lookup_args(target))
        .output()
        .map_err(|source| RitError::transport_io("freedesktop Secret Service", source))?;
    if !output.status.success() {
        return Ok(None);
    }
    let secret = String::from_utf8(output.stdout).map_err(|_| {
        RitError::invalid_input("freedesktop Secret Service returned non-UTF-8 secret")
    })?;
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
    let mut child = Command::new("secret-tool")
        .args(store_args(target, request, credential))
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|source| RitError::transport_io("freedesktop Secret Service", source))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| RitError::invalid_input("secret-tool stdin is unavailable"))?;
    stdin
        .write_all(credential.secret.expose_secret().as_bytes())
        .map_err(|source| RitError::transport_io("freedesktop Secret Service", source))?;
    drop(stdin);

    let status = child
        .wait()
        .map_err(|source| RitError::transport_io("freedesktop Secret Service", source))?;
    if !status.success() {
        return Err(RitError::invalid_input(
            "freedesktop Secret Service credential store failed",
        ));
    }
    Ok(())
}

pub fn erase(target: &str, _request: &CredentialRequest) -> Result<()> {
    let status = Command::new("secret-tool")
        .args(clear_args(target))
        .status()
        .map_err(|source| RitError::transport_io("freedesktop Secret Service", source))?;
    if !status.success() {
        return Ok(());
    }
    Ok(())
}

fn lookup_args(target: &str) -> Vec<String> {
    vec!["lookup".to_owned(), "target".to_owned(), target.to_owned()]
}

fn store_args(target: &str, request: &CredentialRequest, credential: &Credential) -> Vec<String> {
    let username = credential
        .username
        .as_ref()
        .or(request.username.as_ref())
        .cloned()
        .unwrap_or_default();
    vec![
        "store".to_owned(),
        "--label".to_owned(),
        format!("rit credential for {target}"),
        "target".to_owned(),
        target.to_owned(),
        "protocol".to_owned(),
        request.protocol.clone(),
        "host".to_owned(),
        request.host.clone(),
        "path".to_owned(),
        request.path.clone().unwrap_or_default(),
        "username".to_owned(),
        username,
    ]
}

fn clear_args(target: &str) -> Vec<String> {
    vec!["clear".to_owned(), "target".to_owned(), target.to_owned()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn libsecret_arguments_use_target_as_unique_lookup_key() {
        let mut request = CredentialRequest::new("https", "example.test");
        request.path = Some("org/repo.git".to_owned());
        let credential = Credential::password("alice", "secret");

        assert_eq!(
            lookup_args("rit:https://example.test/org/repo.git"),
            vec!["lookup", "target", "rit:https://example.test/org/repo.git"]
        );
        assert_eq!(
            clear_args("rit:https://example.test/org/repo.git"),
            vec!["clear", "target", "rit:https://example.test/org/repo.git"]
        );
        assert!(
            store_args(
                "rit:https://example.test/org/repo.git",
                &request,
                &credential
            )
            .contains(&"alice".to_owned())
        );
    }
}
