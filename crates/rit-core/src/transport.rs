use std::{
    env,
    io::{BufReader, ErrorKind, Read, Write},
    net::TcpStream,
    path::PathBuf,
    process::{Command, Stdio},
    time::Duration,
};

use native_tls::TlsConnector;

use crate::{GitConfig, ObjectId, Result, RitError};

mod receive_pack;
mod upload_pack;

pub use receive_pack::{
    ReceivePackCommand, ReceivePackCommandStatus, ReceivePackRequest, ReceivePackStatus,
};
pub use upload_pack::{
    UploadPackAckStatus, UploadPackAcknowledgement, UploadPackRequest, UploadPackResponse,
    UploadPackSideBand,
};

/// One simple fetch refspec, such as `refs/heads/main:refs/remotes/origin/main`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FetchRefSpec {
    /// Allow non-fast-forward updates. The first implementation records this
    /// bit but does not yet perform ancestry checks.
    pub force: bool,
    /// Source ref or revision in the remote repository.
    pub source: String,
    /// Destination ref to update in the local repository.
    pub destination: String,
}

/// Git smart HTTP service endpoints.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SmartHttpService {
    /// Fetch/clone service.
    UploadPack,
    /// Push service.
    ReceivePack,
}

impl SmartHttpService {
    /// Returns the service name used in Git HTTP query parameters and MIME
    /// types.
    pub fn name(self) -> &'static str {
        match self {
            Self::UploadPack => "git-upload-pack",
            Self::ReceivePack => "git-receive-pack",
        }
    }
}

/// Request metadata for Git smart HTTP reference discovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmartHttpRequest {
    /// URL for `GET` reference discovery.
    pub info_refs_url: String,
    /// Expected advertisement content type.
    pub advertisement_content_type: String,
}

/// Request metadata for a smart HTTP service POST.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmartHttpPostRequest {
    /// URL for the service request.
    pub url: String,
    /// Request content type.
    pub content_type: String,
    /// Expected response content type.
    pub response_content_type: String,
    /// Serialized pkt-line body.
    pub body: Vec<u8>,
}

/// Minimal HTTP response returned by the smart HTTP client.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmartHttpResponse {
    /// Numeric HTTP status code.
    pub status_code: u16,
    /// Response headers in received order.
    pub headers: Vec<(String, String)>,
    /// Raw response body.
    pub body: Vec<u8>,
}

/// Result of a single smart HTTP upload-pack negotiation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemotePackNegotiation {
    /// Advertisement used to choose the wanted object and capabilities.
    pub advertisement: SmartHttpAdvertisement,
    /// Ref requested by the caller.
    pub wanted_ref: String,
    /// Object ID advertised for `wanted_ref`.
    pub want_id: ObjectId,
    /// Parsed upload-pack response returned by the server.
    pub response: UploadPackResponse,
    /// Raw pack bytes extracted from raw or side-band upload-pack data.
    pub pack_bytes: Vec<u8>,
}

/// Command metadata needed to start a Git service over SSH.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SshServiceCommand {
    /// Git service to request on the remote side.
    pub service: SmartHttpService,
    /// Optional SSH username from `user@host` locations.
    pub user: Option<String>,
    /// SSH host name.
    pub host: String,
    /// Optional SSH port from `ssh://host:port/path` URLs.
    pub port: Option<u16>,
    /// Repository path passed to the remote Git service.
    pub path: String,
    /// Remote shell command, such as `git-upload-pack 'repo.git'`.
    pub remote_command: String,
}

/// Local process invocation used for SSH transport.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SshProcessInvocation {
    /// Program to execute, usually `ssh` or `GIT_SSH`.
    pub program: String,
    /// Arguments passed to the SSH program.
    pub args: Vec<String>,
}

/// Repository-level SSH process configuration.
///
/// Environment variables are still read at process start time and take the
/// same precedence as Git: `GIT_SSH_COMMAND`, then `core.sshCommand`, then
/// `GIT_SSH`, finally plain `ssh`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SshProcessConfig {
    /// Command from `core.sshCommand`, in the same shell-like form as
    /// `GIT_SSH_COMMAND`.
    pub core_ssh_command: Option<String>,
}

impl SshProcessConfig {
    /// Reads SSH process settings from parsed Git config.
    pub fn from_git_config(config: &GitConfig) -> Self {
        Self {
            core_ssh_command: config
                .get("core", "sshcommand")
                .filter(|value| !value.trim().is_empty())
                .map(ToOwned::to_owned),
        }
    }
}

impl SshServiceCommand {
    /// Returns the `ssh` destination argument, including `user@` when present.
    pub fn target(&self) -> String {
        match &self.user {
            Some(user) => format!("{user}@{}", self.host),
            None => self.host.clone(),
        }
    }
}

/// Executes an SSH Git service session.
pub trait SshServiceExecutor {
    /// Sends a pkt-line request to the remote command and returns raw response bytes.
    fn run(&self, command: &SshServiceCommand, request: &[u8]) -> Result<Vec<u8>>;
}

/// Executes an interactive SSH upload-pack session.
pub trait SshUploadPackExecutor {
    /// Negotiates one upload-pack request for an advertised ref.
    fn negotiate_upload_pack(
        &self,
        location: &TransportLocation,
        wanted_ref: &str,
        haves: Vec<ObjectId>,
    ) -> Result<RemotePackNegotiation>;
}

/// Executes an interactive SSH receive-pack session.
pub trait SshReceivePackExecutor {
    /// Sends one receive-pack update and parses the report-status response.
    fn send_receive_pack(
        &self,
        location: &TransportLocation,
        ref_name: &str,
        new_id: ObjectId,
        pack_data: Vec<u8>,
    ) -> Result<ReceivePackStatus>;
}

/// SSH executor backed by the system `ssh` program.
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcessSshServiceExecutor;

impl SshServiceExecutor for ProcessSshServiceExecutor {
    fn run(&self, command: &SshServiceCommand, request: &[u8]) -> Result<Vec<u8>> {
        run_ssh_service_process(command, request, &SshProcessConfig::default())
    }
}

impl SshUploadPackExecutor for ProcessSshServiceExecutor {
    fn negotiate_upload_pack(
        &self,
        location: &TransportLocation,
        wanted_ref: &str,
        haves: Vec<ObjectId>,
    ) -> Result<RemotePackNegotiation> {
        negotiate_upload_pack_process(location, wanted_ref, haves, &SshProcessConfig::default())
    }
}

impl SshReceivePackExecutor for ProcessSshServiceExecutor {
    fn send_receive_pack(
        &self,
        location: &TransportLocation,
        ref_name: &str,
        new_id: ObjectId,
        pack_data: Vec<u8>,
    ) -> Result<ReceivePackStatus> {
        send_receive_pack_process(
            location,
            ref_name,
            new_id,
            pack_data,
            &SshProcessConfig::default(),
        )
    }
}

/// SSH executor backed by an explicit repository process configuration.
#[derive(Clone, Debug, Default)]
pub struct ConfiguredProcessSshServiceExecutor {
    process_config: SshProcessConfig,
}

impl ConfiguredProcessSshServiceExecutor {
    /// Builds a process executor from repository-level SSH settings.
    pub fn new(process_config: SshProcessConfig) -> Self {
        Self { process_config }
    }
}

impl SshServiceExecutor for ConfiguredProcessSshServiceExecutor {
    fn run(&self, command: &SshServiceCommand, request: &[u8]) -> Result<Vec<u8>> {
        run_ssh_service_process(command, request, &self.process_config)
    }
}

impl SshUploadPackExecutor for ConfiguredProcessSshServiceExecutor {
    fn negotiate_upload_pack(
        &self,
        location: &TransportLocation,
        wanted_ref: &str,
        haves: Vec<ObjectId>,
    ) -> Result<RemotePackNegotiation> {
        negotiate_upload_pack_process(location, wanted_ref, haves, &self.process_config)
    }
}

impl SshReceivePackExecutor for ConfiguredProcessSshServiceExecutor {
    fn send_receive_pack(
        &self,
        location: &TransportLocation,
        ref_name: &str,
        new_id: ObjectId,
        pack_data: Vec<u8>,
    ) -> Result<ReceivePackStatus> {
        send_receive_pack_process(location, ref_name, new_id, pack_data, &self.process_config)
    }
}

fn run_ssh_service_process(
    command: &SshServiceCommand,
    request: &[u8],
    process_config: &SshProcessConfig,
) -> Result<Vec<u8>> {
    let mut process = command_to_process(command, process_config)?;
    let mut child = process
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| RitError::transport_io(command.host.clone(), source))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(request)
            .map_err(|source| RitError::transport_io(command.host.clone(), source))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|source| RitError::transport_io(command.host.clone(), source))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RitError::invalid_input(format!(
            "SSH service command failed for {}: {}",
            command.host,
            stderr.trim()
        )));
    }
    Ok(output.stdout)
}

fn negotiate_upload_pack_process(
    location: &TransportLocation,
    wanted_ref: &str,
    haves: Vec<ObjectId>,
    process_config: &SshProcessConfig,
) -> Result<RemotePackNegotiation> {
    let command = location.ssh_service_command(SmartHttpService::UploadPack)?;
    let mut process = command_to_process(&command, process_config)?;
    let mut child = process
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| RitError::transport_io(command.host.clone(), source))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| RitError::invalid_input("SSH upload-pack stdout was not captured"))?;
    let mut stdout = BufReader::new(stdout);
    let mut advertisement_bytes = Vec::new();
    read_ssh_advertisement(&mut stdout, &mut advertisement_bytes, &command.host)?;
    let advertisement = SmartHttpAdvertisement::parse_git_protocol(
        SmartHttpService::UploadPack,
        &advertisement_bytes,
    )?;
    let want_id = advertised_ref_id(&advertisement, wanted_ref).ok_or_else(|| {
        RitError::invalid_input(format!(
            "remote did not advertise requested ref: {wanted_ref}"
        ))
    })?;
    let request = UploadPackRequest::new(vec![want_id])?
        .with_capabilities(select_upload_pack_capabilities(&advertisement.capabilities))
        .with_haves(haves);

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(&request.to_pkt_lines())
            .map_err(|source| RitError::transport_io(command.host.clone(), source))?;
    }

    let mut response_bytes = Vec::new();
    stdout
        .read_to_end(&mut response_bytes)
        .map_err(|source| RitError::transport_io(command.host.clone(), source))?;
    let output = child
        .wait_with_output()
        .map_err(|source| RitError::transport_io(command.host.clone(), source))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RitError::invalid_input(format!(
            "SSH upload-pack failed for {}: {}",
            command.host,
            stderr.trim()
        )));
    }

    let response = UploadPackResponse::parse(&response_bytes)?;
    reject_upload_pack_error(&response)?;
    let pack_bytes = response
        .pack_bytes()?
        .ok_or_else(|| RitError::invalid_input("upload-pack response did not include a pack"))?;

    Ok(RemotePackNegotiation {
        advertisement,
        wanted_ref: wanted_ref.to_owned(),
        want_id,
        response,
        pack_bytes,
    })
}

fn send_receive_pack_process(
    location: &TransportLocation,
    ref_name: &str,
    new_id: ObjectId,
    pack_data: Vec<u8>,
    process_config: &SshProcessConfig,
) -> Result<ReceivePackStatus> {
    let command = location.ssh_service_command(SmartHttpService::ReceivePack)?;
    let mut process = command_to_process(&command, process_config)?;
    let mut child = process
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| RitError::transport_io(command.host.clone(), source))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| RitError::invalid_input("SSH receive-pack stdout was not captured"))?;
    let mut stdout = BufReader::new(stdout);
    let mut advertisement_bytes = Vec::new();
    read_ssh_advertisement(&mut stdout, &mut advertisement_bytes, &command.host)?;
    let advertisement = SmartHttpAdvertisement::parse_git_protocol(
        SmartHttpService::ReceivePack,
        &advertisement_bytes,
    )?;
    let old_id = advertised_ref_id(&advertisement, ref_name).unwrap_or_else(zero_object_id);
    let receive_command = ReceivePackCommand::new(old_id, new_id, ref_name.to_owned())?;
    let request = ReceivePackRequest::new(vec![receive_command])?
        .with_capabilities(select_receive_pack_capabilities(
            &advertisement.capabilities,
        ))
        .with_pack_data(pack_data);

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(&request.to_bytes())
            .map_err(|source| RitError::transport_io(command.host.clone(), source))?;
    }

    let mut response_bytes = Vec::new();
    stdout
        .read_to_end(&mut response_bytes)
        .map_err(|source| RitError::transport_io(command.host.clone(), source))?;
    let output = child
        .wait_with_output()
        .map_err(|source| RitError::transport_io(command.host.clone(), source))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RitError::invalid_input(format!(
            "SSH receive-pack failed for {}: {}",
            command.host,
            stderr.trim()
        )));
    }

    ReceivePackStatus::parse(&response_bytes)
}

fn command_to_process(
    command: &SshServiceCommand,
    process_config: &SshProcessConfig,
) -> Result<Command> {
    let git_ssh_command = env::var("GIT_SSH_COMMAND").ok();
    let git_ssh = env::var("GIT_SSH").ok();
    let invocation = ssh_process_invocation(
        command,
        git_ssh_command.as_deref(),
        process_config.core_ssh_command.as_deref(),
        git_ssh.as_deref(),
    )?;
    let mut process = Command::new(&invocation.program);
    process.args(&invocation.args);
    Ok(process)
}

fn ssh_process_invocation(
    command: &SshServiceCommand,
    git_ssh_command: Option<&str>,
    core_ssh_command: Option<&str>,
    git_ssh: Option<&str>,
) -> Result<SshProcessInvocation> {
    let (program, mut args) = match git_ssh_command.filter(|value| !value.trim().is_empty()) {
        Some(command) => parse_git_ssh_command(command)?,
        None => match core_ssh_command.filter(|value| !value.trim().is_empty()) {
            Some(command) => parse_git_ssh_command(command)?,
            None => match git_ssh.filter(|value| !value.trim().is_empty()) {
                Some(program) => (program.to_owned(), Vec::new()),
                None => ("ssh".to_owned(), Vec::new()),
            },
        },
    };
    add_ssh_process_args(&mut args, command);
    Ok(SshProcessInvocation { program, args })
}

fn add_ssh_process_args(args: &mut Vec<String>, command: &SshServiceCommand) {
    if let Some(port) = command.port {
        args.push("-p".to_owned());
        args.push(port.to_string());
    }
    args.push(command.target());
    args.push(command.remote_command.clone());
}

fn parse_git_ssh_command(command: &str) -> Result<(String, Vec<String>)> {
    let words = split_command_words(command)?;
    let Some((program, args)) = words.split_first() else {
        return Err(RitError::invalid_input("GIT_SSH_COMMAND is empty"));
    };
    Ok((program.clone(), args.to_vec()))
}

fn split_command_words(command: &str) -> Result<Vec<String>> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut chars = command.chars();
    let mut quote = None;

    while let Some(ch) = chars.next() {
        match (quote, ch) {
            (None, '\'') => quote = Some('\''),
            (None, '"') => quote = Some('"'),
            (None, ch) if ch.is_whitespace() => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            }
            (Some('\''), '\'') | (Some('"'), '"') => quote = None,
            (Some('"'), '\\') | (None, '\\') => {
                let Some(next) = chars.next() else {
                    return Err(RitError::invalid_input(
                        "GIT_SSH_COMMAND has a trailing backslash",
                    ));
                };
                current.push(next);
            }
            _ => current.push(ch),
        }
    }

    if quote.is_some() {
        return Err(RitError::invalid_input(
            "GIT_SSH_COMMAND has an unterminated quote",
        ));
    }
    if !current.is_empty() {
        words.push(current);
    }
    Ok(words)
}

/// Runs one SSH upload-pack request and parses the server response.
pub fn run_ssh_upload_pack(
    location: &TransportLocation,
    request: &UploadPackRequest,
    executor: &impl SshServiceExecutor,
) -> Result<UploadPackResponse> {
    let command = location.ssh_service_command(SmartHttpService::UploadPack)?;
    let response = executor.run(&command, &request.to_pkt_lines())?;
    UploadPackResponse::parse(&response)
}

fn read_ssh_advertisement(
    reader: &mut impl Read,
    output: &mut Vec<u8>,
    location: &str,
) -> Result<()> {
    loop {
        let line = read_pkt_line_from(reader, location)?;
        let is_flush = line == b"0000";
        output.extend_from_slice(&line);
        if is_flush {
            break;
        }
    }
    Ok(())
}

fn read_pkt_line_from(reader: &mut impl Read, location: &str) -> Result<Vec<u8>> {
    let mut header = [0_u8; 4];
    match reader.read_exact(&mut header) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::UnexpectedEof => {
            return Err(RitError::invalid_input(
                "SSH upload-pack ended before advertising refs",
            ));
        }
        Err(error) => return Err(RitError::transport_io(location.to_owned(), error)),
    }
    let length_text = std::str::from_utf8(&header)
        .map_err(|_| RitError::invalid_input("pkt-line length is not UTF-8"))?;
    let length = u16::from_str_radix(length_text, 16)
        .map_err(|_| RitError::invalid_input("invalid pkt-line length"))? as usize;
    let mut raw = header.to_vec();
    if length == 0 {
        return Ok(raw);
    }
    if length < 4 {
        return Err(RitError::invalid_input(
            "pkt-line length is smaller than header",
        ));
    }
    let mut payload = vec![0_u8; length - 4];
    reader
        .read_exact(&mut payload)
        .map_err(|source| RitError::transport_io(location.to_owned(), source))?;
    raw.extend_from_slice(&payload);
    Ok(raw)
}

/// One reference advertised by a smart HTTP service.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdvertisedRef {
    /// Object ID currently stored in the advertised ref.
    pub object_id: ObjectId,
    /// Full ref name, such as `refs/heads/main`.
    pub name: String,
}

/// Parsed smart HTTP reference advertisement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmartHttpAdvertisement {
    /// Service that produced the advertisement.
    pub service: SmartHttpService,
    /// Capabilities listed on the first ref record.
    pub capabilities: Vec<String>,
    /// Advertised refs in stream order.
    pub refs: Vec<AdvertisedRef>,
}

impl SmartHttpResponse {
    /// Returns a header value with case-insensitive name matching.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(header_name, _)| header_name.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// Small blocking HTTP client for the first smart HTTP implementation.
#[derive(Clone, Debug)]
pub struct BlockingSmartHttpClient {
    timeout: Duration,
}

impl Default for BlockingSmartHttpClient {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
        }
    }
}

impl BlockingSmartHttpClient {
    /// Builds a client with a custom connect/read/write timeout.
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }

    /// Performs smart HTTP reference discovery with a GET request.
    pub fn get_info_refs(
        &self,
        location: &TransportLocation,
        service: SmartHttpService,
    ) -> Result<SmartHttpResponse> {
        let request = location.smart_http_info_refs(service)?;
        let response = self.send_http_request("GET", &request.info_refs_url, None, &[])?;
        validate_smart_http_response(
            &response,
            &request.advertisement_content_type,
            &[200, 304],
            SmartHttpBodyCheck::InfoRefsAdvertisement,
        )?;
        Ok(response)
    }

    /// Discovers and parses refs advertised by a smart HTTP service.
    pub fn discover_refs(
        &self,
        location: &TransportLocation,
        service: SmartHttpService,
    ) -> Result<SmartHttpAdvertisement> {
        let response = self.get_info_refs(location, service)?;
        SmartHttpAdvertisement::parse(service, &response.body)
    }

    /// Performs one smart HTTP upload-pack negotiation for an advertised ref.
    pub fn negotiate_upload_pack(
        &self,
        location: &TransportLocation,
        wanted_ref: &str,
        haves: Vec<ObjectId>,
    ) -> Result<RemotePackNegotiation> {
        let advertisement = self.discover_refs(location, SmartHttpService::UploadPack)?;
        let advertised_ref = advertisement
            .refs
            .iter()
            .find(|advertised_ref| advertised_ref.name == wanted_ref)
            .ok_or_else(|| {
                RitError::invalid_input(format!("remote did not advertise ref: {wanted_ref}"))
            })?;
        let want_id = advertised_ref.object_id;
        let capabilities = select_upload_pack_capabilities(&advertisement.capabilities);
        let request = UploadPackRequest::new(vec![want_id])?
            .with_capabilities(capabilities)
            .with_haves(haves);
        let response = self.post_upload_pack(location, &request)?;
        let parsed_response = UploadPackResponse::parse(&response.body)?;
        reject_upload_pack_error(&parsed_response)?;
        let pack_bytes = parsed_response.pack_bytes()?.ok_or_else(|| {
            RitError::invalid_input("upload-pack response did not include pack data")
        })?;

        Ok(RemotePackNegotiation {
            advertisement,
            wanted_ref: wanted_ref.to_owned(),
            want_id,
            response: parsed_response,
            pack_bytes,
        })
    }

    /// Performs a smart HTTP upload-pack POST request.
    pub fn post_upload_pack(
        &self,
        location: &TransportLocation,
        request: &UploadPackRequest,
    ) -> Result<SmartHttpResponse> {
        let post_request = location.smart_http_upload_pack(request)?;
        let response = self.send_http_request(
            "POST",
            &post_request.url,
            Some(post_request.content_type.as_str()),
            &post_request.body,
        )?;
        validate_smart_http_response(
            &response,
            &post_request.response_content_type,
            &[200],
            SmartHttpBodyCheck::None,
        )?;
        Ok(response)
    }

    /// Performs a smart HTTP receive-pack POST request and parses report-status.
    pub fn post_receive_pack(
        &self,
        location: &TransportLocation,
        request: &ReceivePackRequest,
    ) -> Result<ReceivePackStatus> {
        let post_request = location.smart_http_receive_pack(request)?;
        let response = self.send_http_request(
            "POST",
            &post_request.url,
            Some(post_request.content_type.as_str()),
            &post_request.body,
        )?;
        validate_smart_http_response(
            &response,
            &post_request.response_content_type,
            &[200],
            SmartHttpBodyCheck::None,
        )?;
        ReceivePackStatus::parse(&response.body)
    }

    fn send_http_request(
        &self,
        method: &str,
        url: &str,
        content_type: Option<&str>,
        body: &[u8],
    ) -> Result<SmartHttpResponse> {
        let parsed_url = HttpUrl::parse(url)?;
        let address = format!("{}:{}", parsed_url.host, parsed_url.port);
        let stream = TcpStream::connect(address)
            .map_err(|source| RitError::transport_io(url.to_owned(), source))?;
        stream
            .set_read_timeout(Some(self.timeout))
            .map_err(|source| RitError::transport_io(url.to_owned(), source))?;
        stream
            .set_write_timeout(Some(self.timeout))
            .map_err(|source| RitError::transport_io(url.to_owned(), source))?;

        let request = build_http_request(method, &parsed_url, content_type, body);
        if parsed_url.scheme == HttpScheme::Https {
            let connector = TlsConnector::new()
                .map_err(|source| transport_tls_error(url, "TLS initialization", source))?;
            let mut stream = connector
                .connect(&parsed_url.host, stream)
                .map_err(|source| transport_tls_error(url, "TLS handshake", source))?;
            return send_http_request_bytes(url, &request, &mut stream);
        }

        let mut stream = stream;
        send_http_request_bytes(url, &request, &mut stream)
    }
}

fn send_http_request_bytes(
    url: &str,
    request: &[u8],
    stream: &mut impl ReadWrite,
) -> Result<SmartHttpResponse> {
    stream
        .write_all(request)
        .map_err(|source| RitError::transport_io(url.to_owned(), source))?;
    stream
        .flush()
        .map_err(|source| RitError::transport_io(url.to_owned(), source))?;

    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|source| RitError::transport_io(url.to_owned(), source))?;
    parse_http_response(&response)
}

trait ReadWrite: Read + Write {}

impl<T: Read + Write> ReadWrite for T {}

fn transport_tls_error(
    location: &str,
    context: &str,
    source: impl std::error::Error + Send + Sync + 'static,
) -> RitError {
    RitError::transport_io(
        location.to_owned(),
        std::io::Error::other(format!("{context} failed: {source}")),
    )
}

impl SmartHttpAdvertisement {
    /// Parses the pkt-line response body returned by smart HTTP `info/refs`.
    pub fn parse(service: SmartHttpService, bytes: &[u8]) -> Result<Self> {
        let lines = parse_pkt_lines(bytes)?;
        let mut iter = lines.into_iter();
        let Some(service_line) = iter.next() else {
            return Err(RitError::invalid_input("empty smart HTTP advertisement"));
        };
        let expected = format!("# service={}\n", service.name());
        if service_line != expected.as_bytes() {
            return Err(RitError::invalid_input(
                "smart HTTP service header mismatch",
            ));
        }
        match iter.next() {
            Some(line) if line.is_empty() => {}
            _ => {
                return Err(RitError::invalid_input(
                    "smart HTTP advertisement missing service flush",
                ));
            }
        }

        let mut capabilities = Vec::new();
        let mut refs = Vec::new();
        for line in iter {
            if line.is_empty() {
                break;
            }
            let (advertised_ref, advertised_capabilities) =
                parse_advertised_ref_line(&line, refs.is_empty())?;
            if let Some(advertised_capabilities) = advertised_capabilities {
                capabilities = advertised_capabilities;
            }
            refs.push(advertised_ref);
        }

        Ok(Self {
            service,
            capabilities,
            refs,
        })
    }

    /// Parses the pkt-line advertisement returned by native Git protocol
    /// transports such as SSH.
    pub fn parse_git_protocol(service: SmartHttpService, bytes: &[u8]) -> Result<Self> {
        parse_advertisement_records(service, parse_pkt_lines(bytes)?)
    }
}

impl FetchRefSpec {
    /// Parses one fetch refspec.
    pub fn parse(input: &str) -> Result<Self> {
        let (force, body) = if let Some(rest) = input.strip_prefix('+') {
            (true, rest)
        } else {
            (false, input)
        };
        let Some((source, destination)) = body.split_once(':') else {
            return Err(RitError::invalid_input(format!(
                "unsupported fetch refspec: {input}"
            )));
        };
        if source.is_empty() || destination.is_empty() {
            return Err(RitError::invalid_input(format!(
                "invalid fetch refspec: {input}"
            )));
        }
        Ok(Self {
            force,
            source: source.to_owned(),
            destination: destination.to_owned(),
        })
    }
}

fn select_upload_pack_capabilities(advertised: &[String]) -> Vec<String> {
    let mut selected = Vec::new();
    push_capability_if_advertised(advertised, &mut selected, "multi_ack_detailed");
    if !selected
        .iter()
        .any(|capability| capability == "multi_ack_detailed")
    {
        push_capability_if_advertised(advertised, &mut selected, "multi_ack");
    }
    push_capability_if_advertised(advertised, &mut selected, "side-band-64k");
    push_capability_if_advertised(advertised, &mut selected, "thin-pack");
    push_capability_if_advertised(advertised, &mut selected, "ofs-delta");
    selected
}

fn select_receive_pack_capabilities(advertised: &[String]) -> Vec<String> {
    if advertised
        .iter()
        .any(|capability| capability == "report-status")
    {
        vec!["report-status".to_owned()]
    } else {
        Vec::new()
    }
}

fn push_capability_if_advertised(advertised: &[String], selected: &mut Vec<String>, name: &str) {
    if advertised.iter().any(|capability| capability == name) {
        selected.push(name.to_owned());
    }
}

fn reject_upload_pack_error(response: &UploadPackResponse) -> Result<()> {
    for acknowledgement in &response.acknowledgements {
        if let UploadPackAcknowledgement::Error { message } = acknowledgement {
            return Err(RitError::invalid_input(format!(
                "upload-pack error: {message}"
            )));
        }
    }
    Ok(())
}

fn parse_advertisement_records(
    service: SmartHttpService,
    lines: Vec<Vec<u8>>,
) -> Result<SmartHttpAdvertisement> {
    let mut capabilities = Vec::new();
    let mut refs = Vec::new();
    for line in lines {
        if line.is_empty() {
            break;
        }
        let (advertised_ref, advertised_capabilities) =
            parse_advertised_ref_line(&line, refs.is_empty())?;
        if let Some(advertised_capabilities) = advertised_capabilities {
            capabilities = advertised_capabilities;
        }
        refs.push(advertised_ref);
    }

    Ok(SmartHttpAdvertisement {
        service,
        capabilities,
        refs,
    })
}

fn advertised_ref_id(advertisement: &SmartHttpAdvertisement, wanted_ref: &str) -> Option<ObjectId> {
    advertisement
        .refs
        .iter()
        .find(|advertised| advertised.name == wanted_ref)
        .map(|advertised| advertised.object_id)
        .or_else(|| {
            if wanted_ref == "HEAD" {
                advertisement
                    .refs
                    .first()
                    .map(|advertised| advertised.object_id)
            } else {
                None
            }
        })
}

fn zero_object_id() -> ObjectId {
    ObjectId::from_bytes([0; 20])
}

pub(super) fn parse_pkt_lines(bytes: &[u8]) -> Result<Vec<Vec<u8>>> {
    let mut lines = Vec::new();
    let mut position = 0;
    while position < bytes.len() {
        let (payload, next_position) = read_pkt_line_at(bytes, position)?;
        lines.push(payload);
        position = next_position;
    }
    Ok(lines)
}

pub(super) fn read_pkt_line_at(bytes: &[u8], position: usize) -> Result<(Vec<u8>, usize)> {
    let length_end = position + 4;
    let Some(length_bytes) = bytes.get(position..length_end) else {
        return Err(RitError::invalid_input("truncated pkt-line length"));
    };
    let length_text = std::str::from_utf8(length_bytes)
        .map_err(|_| RitError::invalid_input("pkt-line length is not UTF-8"))?;
    let length = u16::from_str_radix(length_text, 16)
        .map_err(|_| RitError::invalid_input("invalid pkt-line length"))? as usize;

    if length == 0 {
        return Ok((Vec::new(), length_end));
    }
    if length < 4 {
        return Err(RitError::invalid_input(
            "pkt-line length is smaller than header",
        ));
    }
    let payload_start = length_end;
    let payload_len = length - 4;
    let payload_end = payload_start + payload_len;
    let Some(payload) = bytes.get(payload_start..payload_end) else {
        return Err(RitError::invalid_input("truncated pkt-line payload"));
    };
    Ok((payload.to_vec(), payload_end))
}

pub(super) fn write_pkt_line(output: &mut Vec<u8>, payload: &[u8]) {
    let length = payload.len() + 4;
    output.extend_from_slice(format!("{length:04x}").as_bytes());
    output.extend_from_slice(payload);
}

fn parse_advertised_ref_line(
    line: &[u8],
    first_ref: bool,
) -> Result<(AdvertisedRef, Option<Vec<String>>)> {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    let line_text = std::str::from_utf8(line)
        .map_err(|_| RitError::invalid_input("advertised ref is not UTF-8"))?;
    let (record, capabilities) = if first_ref {
        match line_text.split_once('\0') {
            Some((record, capabilities)) => (
                record,
                Some(
                    capabilities
                        .split_whitespace()
                        .map(ToOwned::to_owned)
                        .collect::<Vec<_>>(),
                ),
            ),
            None => (line_text, Some(Vec::new())),
        }
    } else {
        (line_text, None)
    };
    let Some((object_id, name)) = record.split_once(' ') else {
        return Err(RitError::invalid_input("malformed advertised ref"));
    };
    if name.is_empty() {
        return Err(RitError::invalid_input("advertised ref has empty name"));
    }
    Ok((
        AdvertisedRef {
            object_id: ObjectId::from_hex(object_id)?,
            name: name.to_owned(),
        },
        capabilities,
    ))
}

/// Transport protocol family inferred from a repository location.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransportProtocol {
    /// Local filesystem path.
    Local,
    /// Plain HTTP remote.
    Http,
    /// HTTPS remote.
    Https,
    /// SSH remote, including scp-like `host:path` forms.
    Ssh,
}

/// Parsed repository location used to route transport implementations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransportLocation {
    original: String,
    protocol: TransportProtocol,
}

impl TransportLocation {
    /// Classifies a Git repository argument as local, HTTP(S), or SSH.
    pub fn parse(input: &str) -> Self {
        let protocol = if input.starts_with("http://") {
            TransportProtocol::Http
        } else if input.starts_with("https://") {
            TransportProtocol::Https
        } else if input.starts_with("ssh://") || looks_like_scp_location(input) {
            TransportProtocol::Ssh
        } else {
            TransportProtocol::Local
        };
        Self {
            original: input.to_owned(),
            protocol,
        }
    }

    /// Returns the original repository argument.
    pub fn original(&self) -> &str {
        &self.original
    }

    /// Returns the classified transport protocol.
    pub fn protocol(&self) -> TransportProtocol {
        self.protocol
    }

    /// Returns true when this location can be opened directly from the
    /// filesystem.
    pub fn is_local(&self) -> bool {
        self.protocol == TransportProtocol::Local
    }

    /// Returns the local filesystem path for local locations.
    pub fn local_path(&self) -> Option<PathBuf> {
        self.is_local().then(|| PathBuf::from(&self.original))
    }

    /// Builds the smart HTTP `info/refs` discovery request for HTTP(S)
    /// locations.
    pub fn smart_http_info_refs(&self, service: SmartHttpService) -> Result<SmartHttpRequest> {
        match self.protocol {
            TransportProtocol::Http | TransportProtocol::Https => {
                let base = self.original.trim_end_matches('/');
                let service_name = service.name();
                Ok(SmartHttpRequest {
                    info_refs_url: format!("{base}/info/refs?service={service_name}"),
                    advertisement_content_type: format!(
                        "application/x-{service_name}-advertisement"
                    ),
                })
            }
            _ => Err(RitError::invalid_input(format!(
                "smart HTTP requires an HTTP(S) location: {}",
                self.original
            ))),
        }
    }

    /// Builds the smart HTTP upload-pack POST request metadata.
    pub fn smart_http_upload_pack(
        &self,
        request: &UploadPackRequest,
    ) -> Result<SmartHttpPostRequest> {
        self.smart_http_post_request(SmartHttpService::UploadPack, request.to_pkt_lines())
    }

    /// Builds the smart HTTP receive-pack POST request metadata.
    pub fn smart_http_receive_pack(
        &self,
        request: &ReceivePackRequest,
    ) -> Result<SmartHttpPostRequest> {
        self.smart_http_post_request(SmartHttpService::ReceivePack, request.to_bytes())
    }

    fn smart_http_post_request(
        &self,
        service: SmartHttpService,
        body: Vec<u8>,
    ) -> Result<SmartHttpPostRequest> {
        match self.protocol {
            TransportProtocol::Http | TransportProtocol::Https => {
                let base = self.original.trim_end_matches('/');
                let service_name = service.name();
                Ok(SmartHttpPostRequest {
                    url: format!("{base}/{service_name}"),
                    content_type: format!("application/x-{service_name}-request"),
                    response_content_type: format!("application/x-{service_name}-result"),
                    body,
                })
            }
            _ => Err(RitError::invalid_input(format!(
                "smart HTTP requires an HTTP(S) location: {}",
                self.original
            ))),
        }
    }

    /// Builds the remote Git service command for SSH transports.
    pub fn ssh_service_command(&self, service: SmartHttpService) -> Result<SshServiceCommand> {
        if self.protocol != TransportProtocol::Ssh {
            return Err(RitError::invalid_input(format!(
                "SSH transport requires an SSH location: {}",
                self.original
            )));
        }
        let parsed = ParsedSshLocation::parse(&self.original)?;
        let remote_command = format!("{} {}", service.name(), shell_quote(&parsed.path));
        Ok(SshServiceCommand {
            service,
            user: parsed.user,
            host: parsed.host,
            port: parsed.port,
            path: parsed.path,
            remote_command,
        })
    }
}

struct ParsedSshLocation {
    user: Option<String>,
    host: String,
    port: Option<u16>,
    path: String,
}

impl ParsedSshLocation {
    fn parse(input: &str) -> Result<Self> {
        if let Some(rest) = input.strip_prefix("ssh://") {
            parse_ssh_url(rest, input)
        } else {
            parse_scp_like_location(input)
        }
    }
}

fn parse_ssh_url(rest: &str, original: &str) -> Result<ParsedSshLocation> {
    let Some((authority, path_without_slash)) = rest.split_once('/') else {
        return Err(RitError::invalid_input(format!(
            "SSH URL is missing repository path: {original}"
        )));
    };
    let (user, host, port) = parse_ssh_authority(authority, original, true)?;
    if path_without_slash.is_empty() {
        return Err(RitError::invalid_input(format!(
            "SSH URL is missing repository path: {original}"
        )));
    }
    Ok(ParsedSshLocation {
        user,
        host,
        port,
        path: format!("/{path_without_slash}"),
    })
}

fn parse_scp_like_location(input: &str) -> Result<ParsedSshLocation> {
    let Some((authority, path)) = input.split_once(':') else {
        return Err(RitError::invalid_input(format!(
            "unsupported SSH location: {input}"
        )));
    };
    let (user, host, port) = parse_ssh_authority(authority, input, false)?;
    if path.is_empty() {
        return Err(RitError::invalid_input(format!(
            "SSH location is missing repository path: {input}"
        )));
    }
    Ok(ParsedSshLocation {
        user,
        host,
        port,
        path: path.to_owned(),
    })
}

fn parse_ssh_authority(
    authority: &str,
    original: &str,
    allow_port: bool,
) -> Result<(Option<String>, String, Option<u16>)> {
    if authority.is_empty() {
        return Err(RitError::invalid_input(format!(
            "SSH location is missing host: {original}"
        )));
    }
    let (user, host_with_port) = match authority.rsplit_once('@') {
        Some((user, host)) if !user.is_empty() && !host.is_empty() => {
            (Some(user.to_owned()), host.to_owned())
        }
        Some(_) => {
            return Err(RitError::invalid_input(format!(
                "SSH location has invalid user or host: {original}"
            )));
        }
        None => (None, authority.to_owned()),
    };
    let (host, port) = parse_optional_ssh_port(&host_with_port, original, allow_port)?;
    Ok((user, host, port))
}

fn parse_optional_ssh_port(
    host: &str,
    original: &str,
    allow_port: bool,
) -> Result<(String, Option<u16>)> {
    if !allow_port {
        return Ok((host.to_owned(), None));
    }
    let Some((host, port_text)) = host.rsplit_once(':') else {
        return Ok((host.to_owned(), None));
    };
    if host.is_empty()
        || port_text.is_empty()
        || !port_text.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(RitError::invalid_input(format!(
            "SSH URL has invalid port: {original}"
        )));
    }
    let port = port_text
        .parse::<u16>()
        .map_err(|_| RitError::invalid_input(format!("SSH URL has invalid port: {original}")))?;
    Ok((host.to_owned(), Some(port)))
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HttpScheme {
    Http,
    Https,
}

struct HttpUrl {
    scheme: HttpScheme,
    host: String,
    port: u16,
    path_and_query: String,
}

impl HttpUrl {
    fn parse(url: &str) -> Result<Self> {
        let (scheme, default_port, rest) = if let Some(rest) = url.strip_prefix("http://") {
            (HttpScheme::Http, 80, rest)
        } else if let Some(rest) = url.strip_prefix("https://") {
            (HttpScheme::Https, 443, rest)
        } else {
            return Err(RitError::invalid_input(format!(
                "blocking smart HTTP client supports only http:// and https:// URLs: {url}"
            )));
        };
        let (host_port, path_and_query) = match rest.split_once('/') {
            Some((host_port, path)) => (host_port, format!("/{path}")),
            None => (rest, "/".to_owned()),
        };
        if host_port.is_empty() {
            return Err(RitError::invalid_input(format!(
                "HTTP URL is missing a host: {url}"
            )));
        }
        let (host, port) = match host_port.rsplit_once(':') {
            Some((host, port)) if !host.is_empty() => {
                let port = port.parse::<u16>().map_err(|_| {
                    RitError::invalid_input(format!("HTTP URL has invalid port: {url}"))
                })?;
                (host.to_owned(), port)
            }
            _ => (host_port.to_owned(), default_port),
        };
        Ok(Self {
            scheme,
            host,
            port,
            path_and_query,
        })
    }

    fn host_header(&self) -> String {
        let default_port = match self.scheme {
            HttpScheme::Http => 80,
            HttpScheme::Https => 443,
        };
        if self.port == default_port {
            self.host.clone()
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

fn build_http_request(
    method: &str,
    url: &HttpUrl,
    content_type: Option<&str>,
    body: &[u8],
) -> Vec<u8> {
    let mut request = format!(
        "{method} {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: rit/{}\r\nAccept: */*\r\nConnection: close\r\n",
        url.path_and_query,
        url.host_header(),
        crate::version()
    )
    .into_bytes();
    if let Some(content_type) = content_type {
        request.extend_from_slice(format!("Content-Type: {content_type}\r\n").as_bytes());
        request.extend_from_slice(format!("Content-Length: {}\r\n", body.len()).as_bytes());
    }
    request.extend_from_slice(b"\r\n");
    request.extend_from_slice(body);
    request
}

fn parse_http_response(bytes: &[u8]) -> Result<SmartHttpResponse> {
    let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") else {
        return Err(RitError::invalid_input(
            "HTTP response is missing header terminator",
        ));
    };
    let header_bytes = &bytes[..header_end];
    let raw_body = &bytes[header_end + 4..];
    let header_text = std::str::from_utf8(header_bytes)
        .map_err(|_| RitError::invalid_input("HTTP response headers are not UTF-8"))?;
    let mut lines = header_text.split("\r\n");
    let Some(status_line) = lines.next() else {
        return Err(RitError::invalid_input("HTTP response is empty"));
    };
    let status_code = parse_http_status_code(status_line)?;
    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let Some((name, value)) = line.split_once(':') else {
            return Err(RitError::invalid_input("malformed HTTP response header"));
        };
        headers.push((name.trim().to_owned(), value.trim().to_owned()));
    }

    let body = if http_response_is_chunked(&headers) {
        decode_chunked_body(raw_body)?
    } else {
        raw_body.to_vec()
    };

    Ok(SmartHttpResponse {
        status_code,
        headers,
        body,
    })
}

fn http_response_is_chunked(headers: &[(String, String)]) -> bool {
    headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("Transfer-Encoding"))
        .map(|(_, value)| value.to_ascii_lowercase().contains("chunked"))
        .unwrap_or(false)
}

fn decode_chunked_body(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut position = 0;
    loop {
        let size_line_end = find_crlf(bytes, position)
            .ok_or_else(|| RitError::invalid_input("chunked response is missing chunk size"))?;
        let size_line = std::str::from_utf8(&bytes[position..size_line_end])
            .map_err(|_| RitError::invalid_input("chunked response size is not UTF-8"))?;
        let size_text = size_line.split(';').next().unwrap_or(size_line).trim();
        let chunk_size = usize::from_str_radix(size_text, 16)
            .map_err(|_| RitError::invalid_input("chunked response has invalid chunk size"))?;
        position = size_line_end + 2;

        if chunk_size == 0 {
            return consume_chunked_trailers(bytes, position).map(|_| output);
        }

        let chunk_end = position + chunk_size;
        let Some(chunk) = bytes.get(position..chunk_end) else {
            return Err(RitError::invalid_input(
                "chunked response body is truncated",
            ));
        };
        output.extend_from_slice(chunk);
        position = chunk_end;

        if bytes.get(position..position + 2) != Some(b"\r\n") {
            return Err(RitError::invalid_input(
                "chunked response chunk is missing terminator",
            ));
        }
        position += 2;
    }
}

fn consume_chunked_trailers(bytes: &[u8], mut position: usize) -> Result<usize> {
    loop {
        let line_end = find_crlf(bytes, position)
            .ok_or_else(|| RitError::invalid_input("chunked response trailers are truncated"))?;
        if line_end == position {
            return Ok(line_end + 2);
        }
        position = line_end + 2;
    }
}

fn find_crlf(bytes: &[u8], start: usize) -> Option<usize> {
    bytes
        .get(start..)?
        .windows(2)
        .position(|window| window == b"\r\n")
        .map(|offset| start + offset)
}

fn parse_http_status_code(status_line: &str) -> Result<u16> {
    let mut parts = status_line.split_whitespace();
    let Some(version) = parts.next() else {
        return Err(RitError::invalid_input("HTTP response is missing version"));
    };
    if !version.starts_with("HTTP/") {
        return Err(RitError::invalid_input(
            "HTTP response has invalid status line",
        ));
    }
    let Some(code) = parts.next() else {
        return Err(RitError::invalid_input(
            "HTTP response is missing status code",
        ));
    };
    code.parse::<u16>()
        .map_err(|_| RitError::invalid_input("HTTP response has invalid status code"))
}

enum SmartHttpBodyCheck {
    None,
    InfoRefsAdvertisement,
}

fn validate_smart_http_response(
    response: &SmartHttpResponse,
    expected_content_type: &str,
    allowed_status_codes: &[u16],
    body_check: SmartHttpBodyCheck,
) -> Result<()> {
    if !allowed_status_codes.contains(&response.status_code) {
        return Err(RitError::invalid_input(format!(
            "smart HTTP response returned unexpected status code: {}",
            response.status_code
        )));
    }
    validate_smart_http_content_type(response, expected_content_type)?;
    match body_check {
        SmartHttpBodyCheck::None => {}
        SmartHttpBodyCheck::InfoRefsAdvertisement => {
            validate_info_refs_advertisement_prefix(&response.body)?;
        }
    }
    Ok(())
}

fn validate_smart_http_content_type(
    response: &SmartHttpResponse,
    expected_content_type: &str,
) -> Result<()> {
    let Some(content_type) = response.header("Content-Type") else {
        return Err(RitError::invalid_input(
            "smart HTTP response is missing Content-Type",
        ));
    };
    let media_type = content_type
        .split(';')
        .next()
        .unwrap_or(content_type)
        .trim();
    if media_type != expected_content_type {
        return Err(RitError::invalid_input(format!(
            "smart HTTP response has unsupported Content-Type: {content_type}"
        )));
    }
    Ok(())
}

fn validate_info_refs_advertisement_prefix(body: &[u8]) -> Result<()> {
    let Some(prefix) = body.get(..5) else {
        return Err(RitError::invalid_input(
            "smart HTTP info/refs response is too short",
        ));
    };
    if prefix[..4].iter().all(u8::is_ascii_hexdigit) && prefix[4] == b'#' {
        Ok(())
    } else {
        Err(RitError::invalid_input(
            "smart HTTP info/refs response does not start with an advertisement pkt-line",
        ))
    }
}

fn looks_like_scp_location(input: &str) -> bool {
    if is_windows_drive_path(input) {
        return false;
    }
    let Some(colon_index) = input.find(':') else {
        return false;
    };
    let before_colon = &input[..colon_index];
    !before_colon.is_empty()
        && !before_colon.contains('/')
        && !before_colon.contains('\\')
        && input[colon_index + 1..].contains('/')
}

fn is_windows_drive_path(input: &str) -> bool {
    let bytes = input.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

#[cfg(test)]
mod tests {
    use super::{
        BlockingSmartHttpClient, FetchRefSpec, SmartHttpAdvertisement, SmartHttpService,
        SshProcessConfig, SshProcessInvocation, SshServiceCommand, SshServiceExecutor,
        TransportLocation, TransportProtocol, UploadPackAckStatus, UploadPackAcknowledgement,
        UploadPackRequest, UploadPackResponse, UploadPackSideBand,
    };
    use crate::{GitConfig, ObjectId, Result};
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
        time::Duration,
    };

    #[test]
    fn classifies_local_paths() {
        assert_eq!(
            TransportLocation::parse("../repo").protocol(),
            TransportProtocol::Local
        );
        assert_eq!(
            TransportLocation::parse("C:/repo").protocol(),
            TransportProtocol::Local
        );
        assert_eq!(
            TransportLocation::parse("C:\\repo").protocol(),
            TransportProtocol::Local
        );
    }

    #[test]
    fn classifies_http_and_ssh_locations() {
        assert_eq!(
            TransportLocation::parse("http://example.test/repo.git").protocol(),
            TransportProtocol::Http
        );
        assert_eq!(
            TransportLocation::parse("https://example.test/repo.git").protocol(),
            TransportProtocol::Https
        );
        assert_eq!(
            TransportLocation::parse("ssh://example.test/repo.git").protocol(),
            TransportProtocol::Ssh
        );
        assert_eq!(
            TransportLocation::parse("git@example.test:org/repo.git").protocol(),
            TransportProtocol::Ssh
        );
    }

    #[test]
    fn builds_ssh_service_command_for_scp_like_locations() {
        let command = TransportLocation::parse("git@example.test:org/repo.git")
            .ssh_service_command(SmartHttpService::UploadPack)
            .expect("ssh command");

        assert_eq!(command.service, SmartHttpService::UploadPack);
        assert_eq!(command.user.as_deref(), Some("git"));
        assert_eq!(command.host, "example.test");
        assert_eq!(command.port, None);
        assert_eq!(command.path, "org/repo.git");
        assert_eq!(command.remote_command, "git-upload-pack 'org/repo.git'");
        assert_eq!(command.target(), "git@example.test");
    }

    #[test]
    fn builds_ssh_service_command_for_ssh_urls() {
        let command = TransportLocation::parse("ssh://git@example.test/project.git")
            .ssh_service_command(SmartHttpService::ReceivePack)
            .expect("ssh command");

        assert_eq!(command.service, SmartHttpService::ReceivePack);
        assert_eq!(command.user.as_deref(), Some("git"));
        assert_eq!(command.host, "example.test");
        assert_eq!(command.port, None);
        assert_eq!(command.path, "/project.git");
        assert_eq!(command.remote_command, "git-receive-pack '/project.git'");
    }

    #[test]
    fn builds_ssh_service_command_with_url_port() {
        let command = TransportLocation::parse("ssh://git@example.test:2222/project.git")
            .ssh_service_command(SmartHttpService::UploadPack)
            .expect("ssh command");

        assert_eq!(command.user.as_deref(), Some("git"));
        assert_eq!(command.host, "example.test");
        assert_eq!(command.port, Some(2222));
        assert_eq!(command.target(), "git@example.test");
        assert_eq!(command.remote_command, "git-upload-pack '/project.git'");
    }

    #[test]
    fn builds_default_ssh_process_invocation() {
        let command = TransportLocation::parse("git@example.test:org/repo.git")
            .ssh_service_command(SmartHttpService::UploadPack)
            .expect("ssh command");

        let invocation = super::ssh_process_invocation(&command, None, None, None)
            .expect("default invocation should build");

        assert_eq!(
            invocation,
            SshProcessInvocation {
                program: "ssh".to_owned(),
                args: vec![
                    "git@example.test".to_owned(),
                    "git-upload-pack 'org/repo.git'".to_owned(),
                ],
            }
        );
    }

    #[test]
    fn git_ssh_command_overrides_process_and_keeps_port() {
        let command = TransportLocation::parse("ssh://git@example.test:2222/project.git")
            .ssh_service_command(SmartHttpService::ReceivePack)
            .expect("ssh command");

        let invocation = super::ssh_process_invocation(
            &command,
            Some("plink -batch -i 'key file.ppk'"),
            None,
            None,
        )
        .expect("GIT_SSH_COMMAND invocation should build");

        assert_eq!(
            invocation,
            SshProcessInvocation {
                program: "plink".to_owned(),
                args: vec![
                    "-batch".to_owned(),
                    "-i".to_owned(),
                    "key file.ppk".to_owned(),
                    "-p".to_owned(),
                    "2222".to_owned(),
                    "git@example.test".to_owned(),
                    "git-receive-pack '/project.git'".to_owned(),
                ],
            }
        );
    }

    #[test]
    fn git_ssh_overrides_program_without_extra_args() {
        let command = TransportLocation::parse("example.test:org/repo.git")
            .ssh_service_command(SmartHttpService::UploadPack)
            .expect("ssh command");

        let invocation = super::ssh_process_invocation(&command, None, None, Some("custom-ssh"))
            .expect("GIT_SSH invocation should build");

        assert_eq!(invocation.program, "custom-ssh");
        assert_eq!(
            invocation.args,
            vec![
                "example.test".to_owned(),
                "git-upload-pack 'org/repo.git'".to_owned(),
            ]
        );
    }

    #[test]
    fn core_ssh_command_uses_git_ssh_command_form() {
        let command = TransportLocation::parse("ssh://git@example.test:2222/project.git")
            .ssh_service_command(SmartHttpService::UploadPack)
            .expect("ssh command");

        let invocation = super::ssh_process_invocation(
            &command,
            None,
            Some("ssh -i 'key file' -o StrictHostKeyChecking=no"),
            None,
        )
        .expect("core.sshCommand invocation should build");

        assert_eq!(
            invocation,
            SshProcessInvocation {
                program: "ssh".to_owned(),
                args: vec![
                    "-i".to_owned(),
                    "key file".to_owned(),
                    "-o".to_owned(),
                    "StrictHostKeyChecking=no".to_owned(),
                    "-p".to_owned(),
                    "2222".to_owned(),
                    "git@example.test".to_owned(),
                    "git-upload-pack '/project.git'".to_owned(),
                ],
            }
        );
    }

    #[test]
    fn git_ssh_command_overrides_core_ssh_command() {
        let command = TransportLocation::parse("git@example.test:org/repo.git")
            .ssh_service_command(SmartHttpService::UploadPack)
            .expect("ssh command");

        let invocation = super::ssh_process_invocation(
            &command,
            Some("env-ssh -v"),
            Some("config-ssh -q"),
            Some("git-ssh"),
        )
        .expect("GIT_SSH_COMMAND should win");

        assert_eq!(invocation.program, "env-ssh");
        assert!(invocation.args.contains(&"-v".to_owned()));
        assert!(!invocation.args.contains(&"-q".to_owned()));
    }

    #[test]
    fn core_ssh_command_takes_precedence_over_git_ssh() {
        let command = TransportLocation::parse("git@example.test:org/repo.git")
            .ssh_service_command(SmartHttpService::UploadPack)
            .expect("ssh command");

        let invocation =
            super::ssh_process_invocation(&command, None, Some("config-ssh -q"), Some("git-ssh"))
                .expect("core.sshCommand should win over GIT_SSH");

        assert_eq!(invocation.program, "config-ssh");
        assert!(invocation.args.contains(&"-q".to_owned()));
    }

    #[test]
    fn ssh_process_config_reads_core_ssh_command() {
        let config = GitConfig::parse(
            r#"
            [core]
                sshCommand = ssh -i key
            "#,
        )
        .expect("config should parse");

        assert_eq!(
            SshProcessConfig::from_git_config(&config),
            SshProcessConfig {
                core_ssh_command: Some("ssh -i key".to_owned()),
            }
        );
    }

    #[test]
    fn quotes_ssh_repository_paths_for_remote_shells() {
        let command = TransportLocation::parse("example.test:org/repo'with-quote.git")
            .ssh_service_command(SmartHttpService::UploadPack)
            .expect("ssh command");

        assert_eq!(
            command.remote_command,
            "git-upload-pack 'org/repo'\\''with-quote.git'"
        );
    }

    #[test]
    fn ssh_upload_pack_uses_executor_and_parses_response() {
        struct FakeSshExecutor;

        impl SshServiceExecutor for FakeSshExecutor {
            fn run(&self, command: &SshServiceCommand, request: &[u8]) -> Result<Vec<u8>> {
                assert_eq!(command.target(), "git@example.test");
                assert_eq!(command.remote_command, "git-upload-pack 'org/repo.git'");
                assert!(String::from_utf8_lossy(request).contains("want "));
                Ok(b"0008NAK\nPACKmock".to_vec())
            }
        }

        let request = UploadPackRequest::new(vec![
            ObjectId::from_hex("1111111111111111111111111111111111111111")
                .expect("valid object id"),
        ])
        .expect("valid upload-pack request");
        let response = super::run_ssh_upload_pack(
            &TransportLocation::parse("git@example.test:org/repo.git"),
            &request,
            &FakeSshExecutor,
        )
        .expect("ssh upload-pack response should parse");

        assert_eq!(
            response.pack_bytes().expect("pack bytes"),
            Some(b"PACKmock".to_vec())
        );
    }

    #[test]
    fn parses_simple_fetch_refspecs() {
        let refspec =
            FetchRefSpec::parse("+refs/heads/main:refs/remotes/origin/main").expect("valid");
        assert!(refspec.force);
        assert_eq!(refspec.source, "refs/heads/main");
        assert_eq!(refspec.destination, "refs/remotes/origin/main");

        assert!(FetchRefSpec::parse("refs/heads/main").is_err());
        assert!(FetchRefSpec::parse(":refs/heads/main").is_err());
    }

    #[test]
    fn builds_smart_http_discovery_requests() {
        let location = TransportLocation::parse("https://example.test/repo.git/");
        let request = location
            .smart_http_info_refs(SmartHttpService::UploadPack)
            .expect("https supports smart http");
        assert_eq!(
            request.info_refs_url,
            "https://example.test/repo.git/info/refs?service=git-upload-pack"
        );
        assert_eq!(
            request.advertisement_content_type,
            "application/x-git-upload-pack-advertisement"
        );

        assert!(
            TransportLocation::parse("../repo")
                .smart_http_info_refs(SmartHttpService::UploadPack)
                .is_err()
        );
    }

    #[test]
    fn builds_smart_http_upload_pack_requests() {
        let want = ObjectId::from_hex("0a53e9ddeaddad63ad106860237bbf53411d11a7").expect("want");
        let upload_pack = super::UploadPackRequest::new(vec![want]).expect("request");
        let location = TransportLocation::parse("https://example.test/repo.git/");
        let request = location
            .smart_http_upload_pack(&upload_pack)
            .expect("https supports smart http metadata");

        assert_eq!(request.url, "https://example.test/repo.git/git-upload-pack");
        assert_eq!(
            request.content_type,
            "application/x-git-upload-pack-request"
        );
        assert_eq!(
            request.response_content_type,
            "application/x-git-upload-pack-result"
        );
        assert_eq!(request.body, upload_pack.to_pkt_lines());
    }

    #[test]
    fn builds_smart_http_receive_pack_requests() {
        let old_id = ObjectId::from_bytes([0; 20]);
        let new_id = ObjectId::from_hex("0a53e9ddeaddad63ad106860237bbf53411d11a7").expect("new");
        let command =
            super::ReceivePackCommand::new(old_id, new_id, "refs/heads/main").expect("command");
        let receive_pack = super::ReceivePackRequest::new(vec![command]).expect("request");
        let location = TransportLocation::parse("https://example.test/repo.git/");
        let request = location
            .smart_http_receive_pack(&receive_pack)
            .expect("https supports smart http metadata");

        assert_eq!(
            request.url,
            "https://example.test/repo.git/git-receive-pack"
        );
        assert_eq!(
            request.content_type,
            "application/x-git-receive-pack-request"
        );
        assert_eq!(
            request.response_content_type,
            "application/x-git-receive-pack-result"
        );
        assert_eq!(request.body, receive_pack.to_bytes());
    }

    #[test]
    fn blocking_http_client_gets_info_refs() {
        let (base_url, request_handle) = serve_one_http_request(
            b"HTTP/1.1 200 OK\r\nContent-Type: application/x-git-upload-pack-advertisement\r\nContent-Length: 34\r\nConnection: close\r\n\r\n001e# service=git-upload-pack\n0000",
        );
        let location = TransportLocation::parse(&format!("{base_url}/repo.git"));
        let client = BlockingSmartHttpClient::new(Duration::from_secs(2));
        let response = client
            .get_info_refs(&location, SmartHttpService::UploadPack)
            .expect("GET should succeed");
        let request = String::from_utf8(request_handle.join().expect("server thread"))
            .expect("request is UTF-8");

        assert!(
            request.starts_with("GET /repo.git/info/refs?service=git-upload-pack HTTP/1.1\r\n")
        );
        assert!(request.contains("\r\nHost: 127.0.0.1:"));
        assert_eq!(response.status_code, 200);
        assert_eq!(
            response.header("content-type"),
            Some("application/x-git-upload-pack-advertisement")
        );
        assert_eq!(response.body, b"001e# service=git-upload-pack\n0000");
    }

    #[test]
    fn blocking_http_client_posts_upload_pack() {
        let (base_url, request_handle) =
            serve_one_http_request(b"HTTP/1.1 200 OK\r\nContent-Type: application/x-git-upload-pack-result\r\nContent-Length: 8\r\n\r\n0008NAK\n");
        let location = TransportLocation::parse(&format!("{base_url}/repo.git"));
        let want = ObjectId::from_hex("0a53e9ddeaddad63ad106860237bbf53411d11a7").expect("want");
        let upload_pack = super::UploadPackRequest::new(vec![want]).expect("request");
        let client = BlockingSmartHttpClient::new(Duration::from_secs(2));
        let response = client
            .post_upload_pack(&location, &upload_pack)
            .expect("POST should succeed");
        let request = request_handle.join().expect("server thread");
        let request_text =
            String::from_utf8(request.clone()).expect("request headers and body are UTF-8");

        assert!(request_text.starts_with("POST /repo.git/git-upload-pack HTTP/1.1\r\n"));
        assert!(
            request_text.contains("\r\nContent-Type: application/x-git-upload-pack-request\r\n")
        );
        assert!(request.ends_with(&upload_pack.to_pkt_lines()));
        assert_eq!(response.body, b"0008NAK\n");
    }

    #[test]
    fn blocking_http_client_negotiates_upload_pack_for_advertised_ref() {
        let want = ObjectId::from_hex("0a53e9ddeaddad63ad106860237bbf53411d11a7").expect("want");
        let have = ObjectId::from_hex("441b40d833fdfa93eb2908e52742248faf0ee993").expect("have");
        let mut advertisement = Vec::new();
        test_pkt_line(&mut advertisement, b"# service=git-upload-pack\n");
        advertisement.extend_from_slice(b"0000");
        test_pkt_line(
            &mut advertisement,
            format!("{want} HEAD\0multi_ack_detailed side-band-64k ofs-delta agent=git/2.52\n")
                .as_bytes(),
        );
        test_pkt_line(
            &mut advertisement,
            format!("{want} refs/heads/main\n").as_bytes(),
        );
        advertisement.extend_from_slice(b"0000");

        let mut upload_pack = Vec::new();
        test_pkt_line(&mut upload_pack, b"NAK\n");
        let mut pack_side_band = vec![1];
        pack_side_band.extend_from_slice(b"PACKdata");
        test_pkt_line(&mut upload_pack, &pack_side_band);
        upload_pack.extend_from_slice(b"0000");

        let (base_url, request_handle) = serve_http_requests(vec![
            http_response(
                "application/x-git-upload-pack-advertisement",
                &advertisement,
            ),
            http_response("application/x-git-upload-pack-result", &upload_pack),
        ]);
        let location = TransportLocation::parse(&format!("{base_url}/repo.git"));
        let client = BlockingSmartHttpClient::new(Duration::from_secs(2));
        let negotiation = client
            .negotiate_upload_pack(&location, "refs/heads/main", vec![have])
            .expect("negotiation should fetch pack bytes");
        let requests = request_handle.join().expect("server thread");
        let post_request = String::from_utf8(requests[1].clone()).expect("request should be UTF-8");

        assert_eq!(negotiation.wanted_ref, "refs/heads/main");
        assert_eq!(negotiation.want_id, want);
        assert_eq!(negotiation.pack_bytes, b"PACKdata");
        assert!(post_request.starts_with("POST /repo.git/git-upload-pack HTTP/1.1\r\n"));
        assert!(post_request.contains(
            "want 0a53e9ddeaddad63ad106860237bbf53411d11a7 \
             multi_ack_detailed side-band-64k ofs-delta\n"
        ));
        assert!(post_request.contains("have 441b40d833fdfa93eb2908e52742248faf0ee993\n"));
    }

    #[test]
    fn blocking_http_client_posts_receive_pack_and_parses_status() {
        let (base_url, request_handle) = serve_one_http_request(
            b"HTTP/1.1 200 OK\r\nContent-Type: application/x-git-receive-pack-result\r\nConnection: close\r\n\r\n000eunpack ok\n0017ok refs/heads/main\n0000",
        );
        let location = TransportLocation::parse(&format!("{base_url}/repo.git"));
        let old_id = ObjectId::from_bytes([0; 20]);
        let new_id = ObjectId::from_hex("0a53e9ddeaddad63ad106860237bbf53411d11a7").expect("new");
        let command =
            super::ReceivePackCommand::new(old_id, new_id, "refs/heads/main").expect("command");
        let receive_pack = super::ReceivePackRequest::new(vec![command])
            .expect("request")
            .with_capabilities(vec!["report-status".to_owned()]);
        let client = BlockingSmartHttpClient::new(Duration::from_secs(2));
        let status = client
            .post_receive_pack(&location, &receive_pack)
            .expect("POST should succeed");
        let request = request_handle.join().expect("server thread");
        let request_text =
            String::from_utf8(request.clone()).expect("request headers and body are UTF-8");

        assert!(request_text.starts_with("POST /repo.git/git-receive-pack HTTP/1.1\r\n"));
        assert!(
            request_text.contains("\r\nContent-Type: application/x-git-receive-pack-request\r\n")
        );
        assert!(request.ends_with(&receive_pack.to_bytes()));
        assert_eq!(status.unpack_error, None);
        assert_eq!(
            status.commands,
            [super::ReceivePackCommandStatus::Ok {
                ref_name: "refs/heads/main".to_owned(),
            }]
        );
    }

    #[test]
    fn blocking_http_client_decodes_chunked_responses() {
        let (base_url, request_handle) = serve_one_http_request(
            b"HTTP/1.1 200 OK\r\nContent-Type: application/x-git-upload-pack-advertisement\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n22\r\n001e# service=git-upload-pack\n0000\r\n0\r\n\r\n",
        );
        let location = TransportLocation::parse(&format!("{base_url}/repo.git"));
        let client = BlockingSmartHttpClient::new(Duration::from_secs(2));
        let response = client
            .get_info_refs(&location, SmartHttpService::UploadPack)
            .expect("GET should succeed");

        request_handle.join().expect("server thread");
        assert_eq!(response.status_code, 200);
        assert_eq!(response.body, b"001e# service=git-upload-pack\n0000");
    }

    #[test]
    fn blocking_http_client_rejects_wrong_content_type() {
        let (base_url, request_handle) = serve_one_http_request(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 34\r\n\r\n001e# service=git-upload-pack\n0000",
        );
        let location = TransportLocation::parse(&format!("{base_url}/repo.git"));
        let client = BlockingSmartHttpClient::new(Duration::from_secs(2));
        let error = client
            .get_info_refs(&location, SmartHttpService::UploadPack)
            .expect_err("content type should be validated");

        request_handle.join().expect("server thread");
        assert!(error.to_string().contains("Content-Type"));
    }

    #[test]
    fn blocking_http_client_rejects_bad_info_refs_prefix() {
        let (base_url, request_handle) = serve_one_http_request(
            b"HTTP/1.1 200 OK\r\nContent-Type: application/x-git-upload-pack-advertisement\r\nContent-Length: 5\r\n\r\nhello",
        );
        let location = TransportLocation::parse(&format!("{base_url}/repo.git"));
        let client = BlockingSmartHttpClient::new(Duration::from_secs(2));
        let error = client
            .get_info_refs(&location, SmartHttpService::UploadPack)
            .expect_err("advertisement prefix should be validated");

        request_handle.join().expect("server thread");
        assert!(error.to_string().contains("advertisement pkt-line"));
    }

    #[test]
    fn blocking_http_client_discovers_advertised_refs() {
        let (base_url, request_handle) = serve_one_http_request(
            concat!(
                "HTTP/1.1 200 OK\r\n",
                "Content-Type: application/x-git-upload-pack-advertisement\r\n",
                "Connection: close\r\n",
                "\r\n",
                "001e# service=git-upload-pack\n",
                "0000",
                "005195dcfa3633004da0049d3d0fa03f80589cbcaf31 refs/heads/main\0multi_ack thin-pack\n",
                "0000"
            )
            .as_bytes(),
        );
        let location = TransportLocation::parse(&format!("{base_url}/repo.git"));
        let client = BlockingSmartHttpClient::new(Duration::from_secs(2));
        let advertisement = client
            .discover_refs(&location, SmartHttpService::UploadPack)
            .expect("refs should parse");

        request_handle.join().expect("server thread");
        assert_eq!(advertisement.service, SmartHttpService::UploadPack);
        assert_eq!(advertisement.capabilities, ["multi_ack", "thin-pack"]);
        assert_eq!(advertisement.refs.len(), 1);
        assert_eq!(advertisement.refs[0].name, "refs/heads/main");
    }

    #[test]
    fn rejects_truncated_chunked_responses() {
        let error = super::parse_http_response(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nbo",
        )
        .expect_err("chunk is truncated");

        assert!(error.to_string().contains("truncated"));
    }

    #[test]
    fn http_url_parses_https_with_tls_default_port() {
        let url = super::HttpUrl::parse("https://example.test/repo.git/info/refs?service=x")
            .expect("https URL should parse");

        assert_eq!(url.scheme, super::HttpScheme::Https);
        assert_eq!(url.host, "example.test");
        assert_eq!(url.port, 443);
        assert_eq!(url.host_header(), "example.test");
        assert_eq!(url.path_and_query, "/repo.git/info/refs?service=x");
    }

    #[test]
    fn parses_smart_http_advertisements() {
        let body = concat!(
            "001e# service=git-upload-pack\n",
            "0000",
            "005295dcfa3633004da0049d3d0fa03f80589cbcaf31 refs/heads/maint\0multi_ack thin-pack\n",
            "003fd049f6c27a2244e12041955e262a404c7faba355 refs/heads/master\n",
            "0000"
        );

        let advertisement =
            SmartHttpAdvertisement::parse(SmartHttpService::UploadPack, body.as_bytes())
                .expect("advertisement should parse");

        assert_eq!(advertisement.service, SmartHttpService::UploadPack);
        assert_eq!(advertisement.capabilities, ["multi_ack", "thin-pack"]);
        assert_eq!(advertisement.refs.len(), 2);
        assert_eq!(advertisement.refs[0].name, "refs/heads/maint");
        assert_eq!(advertisement.refs[1].name, "refs/heads/master");
    }

    #[test]
    fn rejects_wrong_smart_http_service_header() {
        let body = "001e# service=git-upload-pack\n0000";
        let error = SmartHttpAdvertisement::parse(SmartHttpService::ReceivePack, body.as_bytes())
            .expect_err("service should be checked");

        assert!(error.to_string().contains("service header"));
    }

    #[test]
    fn builds_upload_pack_request_pkt_lines() {
        let want = ObjectId::from_hex("0a53e9ddeaddad63ad106860237bbf53411d11a7").expect("want");
        let have = ObjectId::from_hex("441b40d833fdfa93eb2908e52742248faf0ee993").expect("have");
        let request = super::UploadPackRequest::new(vec![want])
            .expect("request should build")
            .with_capabilities(vec!["multi_ack".to_owned(), "thin-pack".to_owned()])
            .with_haves(vec![have]);

        let body = String::from_utf8(request.to_pkt_lines()).expect("pkt-lines are UTF-8");

        assert_eq!(
            body,
            concat!(
                "0046want 0a53e9ddeaddad63ad106860237bbf53411d11a7 multi_ack thin-pack\n",
                "0032have 441b40d833fdfa93eb2908e52742248faf0ee993\n",
                "0009done\n"
            )
        );
    }

    #[test]
    fn rejects_upload_pack_request_without_wants() {
        let error = super::UploadPackRequest::new(Vec::new()).expect_err("want is required");

        assert!(error.to_string().contains("at least one want"));
    }

    #[test]
    fn builds_receive_pack_request_body() {
        let old_id = ObjectId::from_hex("441b40d833fdfa93eb2908e52742248faf0ee993").expect("old");
        let new_id = ObjectId::from_hex("0a53e9ddeaddad63ad106860237bbf53411d11a7").expect("new");
        let command =
            super::ReceivePackCommand::new(old_id, new_id, "refs/heads/main").expect("command");
        let request = super::ReceivePackRequest::new(vec![command])
            .expect("request")
            .with_capabilities(vec!["report-status".to_owned(), "side-band-64k".to_owned()])
            .with_pack_data(b"PACKdata".to_vec());

        let body = request.to_bytes();
        let command_bytes = &body[..body.len() - b"PACKdata".len()];
        let command_text = String::from_utf8(command_bytes.to_vec()).expect("command text");

        assert!(command_text.contains(
            "441b40d833fdfa93eb2908e52742248faf0ee993 \
             0a53e9ddeaddad63ad106860237bbf53411d11a7 \
             refs/heads/main\0report-status side-band-64k\n"
        ));
        assert!(command_text.ends_with("0000"));
        assert!(body.ends_with(b"PACKdata"));
    }

    #[test]
    fn rejects_empty_receive_pack_requests() {
        let error = super::ReceivePackRequest::new(Vec::new()).expect_err("command is required");

        assert!(error.to_string().contains("at least one command"));
    }

    #[test]
    fn rejects_receive_pack_commands_without_ref_names() {
        let old_id = ObjectId::from_bytes([0; 20]);
        let new_id = ObjectId::from_hex("0a53e9ddeaddad63ad106860237bbf53411d11a7").expect("new");
        let error =
            super::ReceivePackCommand::new(old_id, new_id, "").expect_err("ref is required");

        assert!(error.to_string().contains("ref name"));
    }

    #[test]
    fn parses_receive_pack_status_reports() {
        let status = super::ReceivePackStatus::parse(
            concat!(
                "000eunpack ok\n",
                "0018ok refs/heads/debug\n",
                "002ang refs/heads/master non-fast-forward\n",
                "0000"
            )
            .as_bytes(),
        )
        .expect("status");

        assert_eq!(status.unpack_error, None);
        assert_eq!(
            status.commands,
            [
                super::ReceivePackCommandStatus::Ok {
                    ref_name: "refs/heads/debug".to_owned(),
                },
                super::ReceivePackCommandStatus::Rejected {
                    ref_name: "refs/heads/master".to_owned(),
                    message: "non-fast-forward".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn parses_receive_pack_unpack_errors() {
        let status = super::ReceivePackStatus::parse(
            b"001dunpack index-pack failed\n0017ok refs/heads/main\n0000",
        )
        .expect("status");

        assert_eq!(status.unpack_error, Some("index-pack failed".to_owned()));
    }

    #[test]
    fn rejects_receive_pack_status_without_command_results() {
        let error =
            super::ReceivePackStatus::parse(b"000eunpack ok\n0000").expect_err("command required");

        assert!(error.to_string().contains("no command results"));
    }

    #[test]
    fn parses_upload_pack_nak_and_raw_pack_response() {
        let response =
            UploadPackResponse::parse(b"0008NAK\nPACK\x00\x00\x00\x02payload").expect("response");

        assert_eq!(response.acknowledgements, [UploadPackAcknowledgement::Nak]);
        assert_eq!(
            response.pack_data,
            Some(b"PACK\x00\x00\x00\x02payload".to_vec())
        );
        assert!(response.side_bands.is_empty());
    }

    #[test]
    fn parses_upload_pack_ack_statuses() {
        let first = "7e47fe2bd8d01d481f44d7af0531bd93d3b21c01";
        let second = "74730d410fcb6603ace96f1dc55ea6196122532d";
        let response = UploadPackResponse::parse(
            format!("003aACK {first} continue\n0037ACK {second} ready\n").as_bytes(),
        )
        .expect("response");

        assert_eq!(
            response.acknowledgements,
            [
                UploadPackAcknowledgement::Ack {
                    object_id: ObjectId::from_hex(first).expect("first object"),
                    status: Some(UploadPackAckStatus::Continue),
                },
                UploadPackAcknowledgement::Ack {
                    object_id: ObjectId::from_hex(second).expect("second object"),
                    status: Some(UploadPackAckStatus::Ready),
                },
            ]
        );
        assert_eq!(response.pack_data, None);
        assert!(response.side_bands.is_empty());
    }

    #[test]
    fn parses_upload_pack_error_response() {
        let response = UploadPackResponse::parse(b"0014ERR unknown ref\n").expect("response");

        assert_eq!(
            response.acknowledgements,
            [UploadPackAcknowledgement::Error {
                message: "unknown ref".to_owned(),
            }]
        );
        assert!(response.side_bands.is_empty());
    }

    #[test]
    fn parses_upload_pack_side_band_packets() {
        let response = UploadPackResponse::parse(
            b"0008NAK\n000d\x01PACKdata000e\x02Counting\n000b\x03fatal\n0000",
        )
        .expect("response");

        assert_eq!(response.acknowledgements, [UploadPackAcknowledgement::Nak]);
        assert_eq!(
            response.side_bands,
            [
                UploadPackSideBand::PackData(b"PACKdata".to_vec()),
                UploadPackSideBand::Progress(b"Counting\n".to_vec()),
                UploadPackSideBand::Error(b"fatal\n".to_vec()),
            ]
        );
        assert_eq!(response.pack_data, None);
    }

    #[test]
    fn extracts_raw_upload_pack_bytes() {
        let response =
            UploadPackResponse::parse(b"0008NAK\nPACK\x00\x00\x00\x02payload").expect("response");

        assert_eq!(
            response.pack_bytes().expect("pack bytes"),
            Some(b"PACK\x00\x00\x00\x02payload".to_vec())
        );
    }

    #[test]
    fn combines_side_band_pack_bytes() {
        let response =
            UploadPackResponse::parse(b"0008NAK\n000a\x01PACKa0008\x02ok\n0009\x01tail0000")
                .expect("response");

        assert_eq!(
            response.pack_bytes().expect("pack bytes"),
            Some(b"PACKatail".to_vec())
        );
    }

    #[test]
    fn reports_side_band_errors_before_pack_application() {
        let response = UploadPackResponse::parse(b"000b\x03fatal\n0000").expect("response");
        let error = response
            .pack_bytes()
            .expect_err("side-band error should fail");

        assert!(error.to_string().contains("fatal"));
    }

    #[test]
    fn rejects_unknown_upload_pack_response_lines() {
        let error =
            UploadPackResponse::parse(b"000dprogress\n").expect_err("line should be rejected");

        assert!(error.to_string().contains("unsupported upload-pack"));
    }

    fn serve_one_http_request(response: &'static [u8]) -> (String, thread::JoinHandle<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let address = listener.local_addr().expect("local address");
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept connection");
            let mut request = Vec::new();
            let mut buffer = [0_u8; 1024];
            loop {
                let bytes_read = stream.read(&mut buffer).expect("read request");
                if bytes_read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..bytes_read]);
                if request_is_complete(&request) {
                    break;
                }
            }
            stream.write_all(response).expect("write response");
            request
        });
        (format!("http://127.0.0.1:{}", address.port()), handle)
    }

    fn serve_http_requests(responses: Vec<Vec<u8>>) -> (String, thread::JoinHandle<Vec<Vec<u8>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let address = listener.local_addr().expect("local address");
        let handle = thread::spawn(move || {
            let mut requests = Vec::new();
            for response in responses {
                let (mut stream, _) = listener.accept().expect("accept connection");
                let mut request = Vec::new();
                let mut buffer = [0_u8; 1024];
                loop {
                    let bytes_read = stream.read(&mut buffer).expect("read request");
                    if bytes_read == 0 {
                        break;
                    }
                    request.extend_from_slice(&buffer[..bytes_read]);
                    if request_is_complete(&request) {
                        break;
                    }
                }
                stream.write_all(&response).expect("write response");
                requests.push(request);
            }
            requests
        });
        (format!("http://127.0.0.1:{}", address.port()), handle)
    }

    fn http_response(content_type: &str, body: &[u8]) -> Vec<u8> {
        let mut response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .into_bytes();
        response.extend_from_slice(body);
        response
    }

    fn test_pkt_line(output: &mut Vec<u8>, payload: &[u8]) {
        let length = payload.len() + 4;
        output.extend_from_slice(format!("{length:04x}").as_bytes());
        output.extend_from_slice(payload);
    }

    fn request_is_complete(request: &[u8]) -> bool {
        let Some(header_end) = request
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .map(|index| index + 4)
        else {
            return false;
        };
        let header_text = String::from_utf8_lossy(&request[..header_end]);
        let Some(content_length) = header_text.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("Content-Length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        }) else {
            return true;
        };
        request.len() >= header_end + content_length
    }
}
