use super::{SshAgentClient, SshAgentIdentity, SshAgentSignFlags, SshAgentSignature};
use std::io::{Cursor, Read, Write};

const SSH_AGENT_FAILURE: u8 = 5;
const SSH_AGENTC_REQUEST_IDENTITIES: u8 = 11;
const SSH_AGENT_IDENTITIES_ANSWER: u8 = 12;
const SSH_AGENTC_SIGN_REQUEST: u8 = 13;
const SSH_AGENT_SIGN_RESPONSE: u8 = 14;

#[derive(Debug)]
struct ScriptedStream {
    read: Cursor<Vec<u8>>,
    written: Vec<u8>,
}

impl ScriptedStream {
    fn new(read: Vec<u8>) -> Self {
        Self {
            read: Cursor::new(read),
            written: Vec::new(),
        }
    }
}

impl Read for ScriptedStream {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.read.read(buffer)
    }
}

impl Write for ScriptedStream {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.written.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn lists_identities_and_writes_request_frame() {
    let mut response = vec![SSH_AGENT_IDENTITIES_ANSWER];
    response.extend_from_slice(&1_u32.to_be_bytes());
    push_bytes(&mut response, b"key-blob");
    push_bytes(&mut response, b"alice@example");

    let stream = ScriptedStream::new(frame(&response));
    let mut client = SshAgentClient::new(stream);

    let identities = client.identities().expect("identities should parse");
    let stream = client.into_inner();

    assert_eq!(
        identities,
        vec![SshAgentIdentity {
            key_blob: b"key-blob".to_vec(),
            comment: "alice@example".to_owned(),
        }]
    );
    assert_eq!(stream.written, frame(&[SSH_AGENTC_REQUEST_IDENTITIES]));
}

#[test]
fn signs_data_and_writes_sign_request_frame() {
    let identity = SshAgentIdentity {
        key_blob: b"key-blob".to_vec(),
        comment: "alice@example".to_owned(),
    };
    let mut response = vec![SSH_AGENT_SIGN_RESPONSE];
    push_bytes(&mut response, b"signature");

    let stream = ScriptedStream::new(frame(&response));
    let mut client = SshAgentClient::new(stream);

    let signature = client
        .sign(&identity, b"payload", SshAgentSignFlags::RSA_SHA2_256)
        .expect("signature should parse");
    let stream = client.into_inner();

    let mut expected_request = vec![SSH_AGENTC_SIGN_REQUEST];
    push_bytes(&mut expected_request, b"key-blob");
    push_bytes(&mut expected_request, b"payload");
    expected_request.extend_from_slice(&2_u32.to_be_bytes());

    assert_eq!(
        signature,
        SshAgentSignature {
            blob: b"signature".to_vec(),
        }
    );
    assert_eq!(stream.written, frame(&expected_request));
}

#[test]
fn reports_agent_failure_without_secret_material() {
    let stream = ScriptedStream::new(frame(&[SSH_AGENT_FAILURE]));
    let mut client = SshAgentClient::new(stream);

    let error = client.identities().expect_err("failure should be reported");

    assert_eq!(error.to_string(), "SSH agent returned failure");
}

fn push_bytes(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    output.extend_from_slice(bytes);
}

fn frame(payload: &[u8]) -> Vec<u8> {
    let mut framed = Vec::new();
    framed.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    framed.extend_from_slice(payload);
    framed
}
