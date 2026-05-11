use crate::{Result, RitError};
use std::io::{Read, Write};

const SSH_AGENT_FAILURE: u8 = 5;
const SSH_AGENTC_REQUEST_IDENTITIES: u8 = 11;
const SSH_AGENT_IDENTITIES_ANSWER: u8 = 12;
const SSH_AGENTC_SIGN_REQUEST: u8 = 13;
const SSH_AGENT_SIGN_RESPONSE: u8 = 14;
const MAX_AGENT_MESSAGE_SIZE: usize = 16 * 1024 * 1024;

/// Public key identity advertised by an SSH agent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SshAgentIdentity {
    /// SSH public key blob in wire format.
    pub key_blob: Vec<u8>,
    /// Human-readable comment supplied by the agent.
    pub comment: String,
}

/// Signature blob returned by an SSH agent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SshAgentSignature {
    /// SSH signature blob in wire format.
    pub blob: Vec<u8>,
}

/// Optional flags for SSH agent signing.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SshAgentSignFlags {
    bits: u32,
}

impl SshAgentSignFlags {
    /// No special signing flags.
    pub const NONE: Self = Self { bits: 0 };
    /// Request RSA-SHA2-256 signatures when supported by the agent.
    pub const RSA_SHA2_256: Self = Self { bits: 2 };
    /// Request RSA-SHA2-512 signatures when supported by the agent.
    pub const RSA_SHA2_512: Self = Self { bits: 4 };

    /// Creates flags from raw protocol bits.
    pub fn from_bits(bits: u32) -> Self {
        Self { bits }
    }

    /// Returns raw protocol bits.
    pub fn bits(self) -> u32 {
        self.bits
    }
}

/// SSH agent protocol client over any blocking read/write stream.
#[derive(Debug)]
pub struct SshAgentClient<S> {
    stream: S,
}

impl<S> SshAgentClient<S> {
    /// Wraps an already-connected SSH agent stream.
    pub fn new(stream: S) -> Self {
        Self { stream }
    }

    /// Returns the wrapped stream.
    pub fn into_inner(self) -> S {
        self.stream
    }
}

impl<S: Read + Write> SshAgentClient<S> {
    /// Requests public identities from the SSH agent.
    pub fn identities(&mut self) -> Result<Vec<SshAgentIdentity>> {
        self.write_frame(&[SSH_AGENTC_REQUEST_IDENTITIES])?;
        parse_identities_answer(&self.read_frame()?)
    }

    /// Requests a signature for `data` using `identity`.
    pub fn sign(
        &mut self,
        identity: &SshAgentIdentity,
        data: &[u8],
        flags: SshAgentSignFlags,
    ) -> Result<SshAgentSignature> {
        let mut payload = vec![SSH_AGENTC_SIGN_REQUEST];
        push_bytes(&mut payload, &identity.key_blob);
        push_bytes(&mut payload, data);
        payload.extend_from_slice(&flags.bits().to_be_bytes());

        self.write_frame(&payload)?;
        parse_sign_response(&self.read_frame()?)
    }

    fn write_frame(&mut self, payload: &[u8]) -> Result<()> {
        let length = u32::try_from(payload.len())
            .map_err(|_| RitError::invalid_input("SSH agent request is too large"))?;
        self.stream
            .write_all(&length.to_be_bytes())
            .and_then(|_| self.stream.write_all(payload))
            .map_err(|source| RitError::transport_io("ssh-agent", source))
    }

    fn read_frame(&mut self) -> Result<Vec<u8>> {
        let mut length = [0; 4];
        self.stream
            .read_exact(&mut length)
            .map_err(|source| RitError::transport_io("ssh-agent", source))?;
        let length = u32::from_be_bytes(length) as usize;
        if length > MAX_AGENT_MESSAGE_SIZE {
            return Err(RitError::invalid_input(format!(
                "SSH agent response is too large: {length} bytes"
            )));
        }

        let mut payload = vec![0; length];
        self.stream
            .read_exact(&mut payload)
            .map_err(|source| RitError::transport_io("ssh-agent", source))?;
        Ok(payload)
    }
}

#[cfg(unix)]
impl SshAgentClient<std::os::unix::net::UnixStream> {
    /// Connects to the Unix-domain socket described by `SSH_AUTH_SOCK`.
    pub fn connect(config: &super::SshAgentConfig) -> Result<Self> {
        let socket = config
            .socket
            .as_ref()
            .ok_or_else(|| RitError::invalid_input("SSH_AUTH_SOCK is not configured"))?;
        let stream = std::os::unix::net::UnixStream::connect(socket)
            .map_err(|source| RitError::io(socket.clone(), source))?;
        Ok(Self::new(stream))
    }
}

fn parse_identities_answer(payload: &[u8]) -> Result<Vec<SshAgentIdentity>> {
    let mut reader = AgentPayloadReader::new(payload);
    match reader.read_byte()? {
        SSH_AGENT_IDENTITIES_ANSWER => {}
        SSH_AGENT_FAILURE => return Err(RitError::invalid_input("SSH agent returned failure")),
        message_type => {
            return Err(RitError::invalid_input(format!(
                "unexpected SSH agent identities response type: {message_type}"
            )));
        }
    }

    let count = reader.read_u32()? as usize;
    let mut identities = Vec::with_capacity(count);
    for _ in 0..count {
        let key_blob = reader.read_bytes()?;
        let comment = String::from_utf8_lossy(&reader.read_bytes()?).into_owned();
        identities.push(SshAgentIdentity { key_blob, comment });
    }
    Ok(identities)
}

fn parse_sign_response(payload: &[u8]) -> Result<SshAgentSignature> {
    let mut reader = AgentPayloadReader::new(payload);
    match reader.read_byte()? {
        SSH_AGENT_SIGN_RESPONSE => Ok(SshAgentSignature {
            blob: reader.read_bytes()?,
        }),
        SSH_AGENT_FAILURE => Err(RitError::invalid_input("SSH agent returned failure")),
        message_type => Err(RitError::invalid_input(format!(
            "unexpected SSH agent sign response type: {message_type}"
        ))),
    }
}

fn push_bytes(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    output.extend_from_slice(bytes);
}

struct AgentPayloadReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> AgentPayloadReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn read_byte(&mut self) -> Result<u8> {
        if self.offset >= self.bytes.len() {
            return Err(RitError::invalid_input("truncated SSH agent response"));
        }
        let byte = self.bytes[self.offset];
        self.offset += 1;
        Ok(byte)
    }

    fn read_u32(&mut self) -> Result<u32> {
        let bytes = self.read_fixed(4)?;
        Ok(u32::from_be_bytes(
            bytes.try_into().expect("fixed read returns 4 bytes"),
        ))
    }

    fn read_bytes(&mut self) -> Result<Vec<u8>> {
        let length = self.read_u32()? as usize;
        Ok(self.read_fixed(length)?.to_vec())
    }

    fn read_fixed(&mut self, length: usize) -> Result<&'a [u8]> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or_else(|| RitError::invalid_input("SSH agent response length overflowed usize"))?;
        if end > self.bytes.len() {
            return Err(RitError::invalid_input("truncated SSH agent response"));
        }

        let bytes = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(bytes)
    }
}
