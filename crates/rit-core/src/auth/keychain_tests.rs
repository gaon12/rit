use super::*;

#[test]
fn keychain_target_includes_protocol_host_and_path() {
    let mut request = CredentialRequest::new("https", "example.test");
    request.path = Some("org/repo.git".to_owned());

    assert_eq!(
        keychain_target(&request),
        "rit:https://example.test/org/repo.git"
    );
}

#[cfg(windows)]
#[test]
fn windows_keychain_round_trips_a_test_credential() {
    let mut request = CredentialRequest::new("https", "example.test");
    request.path = Some(format!(
        "rit-test-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos()
    ));
    let provider = SystemKeychainProvider::current_platform();

    provider
        .store(&request, &Credential::password("alice", "secret"))
        .expect("credential should store");
    let credential = provider
        .credential(&request)
        .expect("credential should read")
        .expect("credential should exist");
    provider.erase(&request).expect("credential should erase");

    assert_eq!(credential.username.as_deref(), Some("alice"));
    assert_eq!(credential.secret.expose_secret(), "secret");
}
