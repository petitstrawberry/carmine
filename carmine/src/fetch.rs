use std::io::Read;
use std::sync::{Mutex, OnceLock};

use encoding_rs::{Encoding, UTF_8, UTF_16BE, UTF_16LE, WINDOWS_1252};

#[cfg(target_os = "scarlet")]
use std::io::{self, Write};

#[cfg(target_os = "scarlet")]
use core::time::Duration;

#[cfg(target_os = "scarlet")]
use scarlet_os::handle::capability::StreamError;

#[cfg(target_os = "scarlet")]
use scarlet_os::socket::{Inet4SocketAddress, Socket, SocketDomain, SocketProtocol, SocketType};

#[cfg(target_os = "scarlet")]
use std::sync::Arc;

#[cfg(target_os = "scarlet")]
const RESOLVERD_SOCKET_PATH: &str = "/tmp/resolverd.sock";

#[cfg(target_os = "scarlet")]
const MAX_REDIRECTS: usize = 8;

pub trait HttpBackend: Send + Sync {
    fn fetch_text(&self, url: &str) -> Result<String, String>;
    fn fetch_bytes(&self, url: &str) -> Result<Vec<u8>, String>;
}

#[cfg(not(target_os = "scarlet"))]
struct UreqBackend;

#[cfg(target_os = "scarlet")]
struct ScarletBackend;

#[cfg(target_os = "scarlet")]
struct ScarletStream {
    socket: Socket,
}

#[cfg(target_os = "scarlet")]
struct HttpResponse {
    status_code: u16,
    headers: String,
    body: Vec<u8>,
}

#[cfg(target_os = "scarlet")]
#[derive(Debug)]
struct ScarletTimeProvider;

#[cfg(target_os = "scarlet")]
impl rustls::time_provider::TimeProvider for ScarletTimeProvider {
    fn current_time(&self) -> Option<rustls_pki_types::UnixTime> {
        let time = std::env::var("CARMINE_TLS_UNIX_TIME")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .map(Duration::from_secs)
            .or_else(scarlet_os::time::system_time)?;
        Some(rustls_pki_types::UnixTime::since_unix_epoch(time))
    }
}

#[cfg(target_os = "scarlet")]
impl ScarletStream {
    fn new(socket: Socket) -> Self {
        Self { socket }
    }
}

#[cfg(target_os = "scarlet")]
impl Read for ScarletStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let stream = self
            .socket
            .as_stream()
            .map_err(|_| io::Error::other("socket is not a stream"))?;
        match stream.read(buf) {
            Ok(n) => Ok(n),
            Err(StreamError::EndOfStream) => Ok(0),
            Err(error) => Err(stream_error_to_io(error)),
        }
    }
}

#[cfg(target_os = "scarlet")]
impl Write for ScarletStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let stream = self
            .socket
            .as_stream()
            .map_err(|_| io::Error::other("socket is not a stream"))?;
        stream.write(buf).map_err(stream_error_to_io)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(target_os = "scarlet")]
fn stream_error_to_io(error: StreamError) -> io::Error {
    match error {
        StreamError::WouldBlock => io::Error::new(io::ErrorKind::WouldBlock, "would block"),
        StreamError::Interrupted => io::Error::new(io::ErrorKind::Interrupted, "interrupted"),
        StreamError::EndOfStream => io::Error::new(io::ErrorKind::UnexpectedEof, "end of stream"),
        StreamError::PermissionDenied => {
            io::Error::new(io::ErrorKind::PermissionDenied, "permission denied")
        }
        StreamError::InvalidParameter => {
            io::Error::new(io::ErrorKind::InvalidInput, "invalid parameter")
        }
        StreamError::Unsupported => io::Error::new(io::ErrorKind::Unsupported, "unsupported"),
        StreamError::IoError | StreamError::SystemError(_) | StreamError::InvalidHandle => {
            io::Error::other(format!("stream error: {:?}", error))
        }
    }
}

pub fn fetch_url(url: &str) -> Result<String, String> {
    with_backend(|backend| backend.fetch_text(url))
}

pub fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
    with_backend(|backend| backend.fetch_bytes(url))
}

fn with_backend<T>(f: impl FnOnce(&dyn HttpBackend) -> Result<T, String>) -> Result<T, String> {
    let slot = backend_slot()
        .lock()
        .map_err(|_| "http backend lock poisoned".to_string())?;
    f(slot.as_ref())
}

fn backend_slot() -> &'static Mutex<Box<dyn HttpBackend>> {
    static BACKEND: OnceLock<Mutex<Box<dyn HttpBackend>>> = OnceLock::new();
    BACKEND.get_or_init(|| Mutex::new(default_backend()))
}

fn default_backend() -> Box<dyn HttpBackend> {
    #[cfg(target_os = "scarlet")]
    {
        Box::new(ScarletBackend)
    }

    #[cfg(not(target_os = "scarlet"))]
    {
        Box::new(UreqBackend)
    }
}

#[cfg(not(target_os = "scarlet"))]
impl HttpBackend for UreqBackend {
    fn fetch_text(&self, url: &str) -> Result<String, String> {
        let resp = ureq::get(url)
            .set("User-Agent", "carmine/0.1")
            .set("Accept", "text/html,*/*;q=0.8")
            .call()
            .map_err(|e| format!("request failed: {}", e))?;
        let content_type = resp.header("Content-Type").map(str::to_string);

        let mut buf = Vec::new();
        resp.into_reader()
            .read_to_end(&mut buf)
            .map_err(|e| format!("read body: {}", e))?;
        decode_html_bytes(&buf, content_type.as_deref())
    }

    fn fetch_bytes(&self, url: &str) -> Result<Vec<u8>, String> {
        let resp = ureq::get(url)
            .set("User-Agent", "carmine/0.1")
            .set("Accept", "*/*")
            .call()
            .map_err(|e| format!("request failed: {}", e))?;

        let mut buf = Vec::new();
        resp.into_reader()
            .read_to_end(&mut buf)
            .map_err(|e| format!("read body: {}", e))?;
        Ok(buf)
    }
}

#[cfg(target_os = "scarlet")]
impl HttpBackend for ScarletBackend {
    fn fetch_text(&self, url: &str) -> Result<String, String> {
        let response = fetch_with_scarlet_socket_response(url)?;
        let content_type = header_value(&response.headers, "content-type");
        decode_html_bytes(&response.body, content_type.as_deref())
    }

    fn fetch_bytes(&self, url: &str) -> Result<Vec<u8>, String> {
        fetch_with_scarlet_socket_response(url).map(|response| response.body)
    }
}

#[cfg(target_os = "scarlet")]
fn fetch_with_scarlet_socket_response(url: &str) -> Result<HttpResponse, String> {
    fetch_with_scarlet_socket_redirects(url, 0)
}

#[cfg(target_os = "scarlet")]
fn fetch_with_scarlet_socket_redirects(
    url: &str,
    redirect_count: usize,
) -> Result<HttpResponse, String> {
    if redirect_count > MAX_REDIRECTS {
        return Err("too many redirects".to_string());
    }

    let parsed = url::Url::parse(url).map_err(|e| format!("invalid URL: {}", e))?;
    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(format!("unsupported URL scheme: {}", scheme));
    }

    let host = parsed
        .host_str()
        .ok_or_else(|| "missing URL host".to_string())?;
    let port = parsed
        .port_or_known_default()
        .ok_or_else(|| format!("missing port for scheme: {}", scheme))?;
    let path = request_target(&parsed);
    let host_header = host_header(&parsed, host, port);
    let socket = connect_scarlet_tcp(host, port)?;

    let response = if scheme == "https" {
        fetch_https_over_scarlet_socket(socket, host, &host_header, &path)
    } else {
        fetch_http_over_stream(socket, &host_header, &path)
    }?;

    if is_redirect(response.status_code) {
        let location = header_value(&response.headers, "location")
            .ok_or_else(|| format!("HTTP redirect {} missing Location", response.status_code))?;
        let next = parsed
            .join(&location)
            .map_err(|e| format!("invalid redirect location: {}", e))?;
        return fetch_with_scarlet_socket_redirects(next.as_str(), redirect_count + 1);
    }

    if response.status_code != 200 {
        return Err(format!("HTTP request failed: {}", response.status_code));
    }

    Ok(response)
}

fn decode_html_bytes(bytes: &[u8], content_type: Option<&str>) -> Result<String, String> {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return decode_with_encoding(UTF_8, &bytes[3..]);
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return decode_with_encoding(UTF_16LE, &bytes[2..]);
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return decode_with_encoding(UTF_16BE, &bytes[2..]);
    }

    let encoding = content_type
        .and_then(charset_from_content_type)
        .or_else(|| charset_from_meta(bytes))
        .and_then(|label| Encoding::for_label(label.as_bytes()))
        .unwrap_or(WINDOWS_1252);

    decode_with_encoding(encoding, bytes)
}

fn decode_with_encoding(encoding: &'static Encoding, bytes: &[u8]) -> Result<String, String> {
    let (text, _, had_errors) = encoding.decode(bytes);
    if had_errors && encoding == UTF_8 {
        return Err("decode body: invalid UTF-8".to_string());
    }
    Ok(text.into_owned())
}

fn charset_from_content_type(content_type: &str) -> Option<String> {
    content_type
        .split(';')
        .find_map(|part| {
            let part = part.trim();
            let (name, value) = part.split_once('=')?;
            if name.trim().eq_ignore_ascii_case("charset") {
                Some(value)
            } else {
                None
            }
        })
        .map(clean_charset_label)
        .filter(|label| !label.is_empty())
}

fn charset_from_meta(bytes: &[u8]) -> Option<String> {
    let limit = bytes.len().min(4096);
    let head = ascii_lowercase(&bytes[..limit]);
    if let Some(pos) = head.find("charset=") {
        return read_charset_after(&head[pos + "charset=".len()..]);
    }
    None
}

fn ascii_lowercase(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| {
            if byte.is_ascii() {
                (*byte as char).to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect()
}

fn read_charset_after(input: &str) -> Option<String> {
    let trimmed = input.trim_start_matches(|ch: char| ch == ' ' || ch == '\'' || ch == '"');
    let label: String = trimmed
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
        .collect();
    let label = clean_charset_label(&label);
    if label.is_empty() { None } else { Some(label) }
}

fn clean_charset_label(label: &str) -> String {
    label
        .trim()
        .trim_matches(|ch| ch == '"' || ch == '\'')
        .to_ascii_lowercase()
}

#[cfg(target_os = "scarlet")]
fn connect_scarlet_tcp(host: &str, port: u16) -> Result<ScarletStream, String> {
    let addrs = lookup_ipv4(host).map_err(|e| format!("resolve {}: {}", host, e))?;

    let mut last_error = None;
    for addr in addrs {
        let socket =
            Socket::new_with_domain(SocketDomain::Inet4, SocketType::Stream, SocketProtocol::Tcp)
                .map_err(|e| format!("socket create: {:?}", e))?;

        let target = Inet4SocketAddress::new(addr, port);
        match socket.connect_inet(target) {
            Ok(()) => return Ok(ScarletStream::new(socket)),
            Err(e) => last_error = Some(format!("connect {}:{}: {:?}", format_ipv4(addr), port, e)),
        }
    }

    Err(last_error.unwrap_or_else(|| format!("no IPv4 address for {}", host)))
}

#[cfg(target_os = "scarlet")]
fn fetch_https_over_scarlet_socket(
    socket: ScarletStream,
    host: &str,
    host_header: &str,
    path: &str,
) -> Result<HttpResponse, String> {
    let root_store =
        rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder_with_details(
        rustls::crypto::ring::default_provider().into(),
        Arc::new(ScarletTimeProvider),
    )
    .with_protocol_versions(&[&rustls::version::TLS12, &rustls::version::TLS13])
    .map_err(|e| format!("TLS protocol setup: {}", e))?
    .with_root_certificates(root_store)
    .with_no_client_auth();
    let server_name = rustls_pki_types::ServerName::try_from(host.to_string())
        .map_err(|e| format!("invalid TLS server name: {}", e))?;
    let connection = rustls::ClientConnection::new(Arc::new(config), server_name)
        .map_err(|e| format!("TLS client setup: {}", e))?;
    let mut stream = rustls::StreamOwned::new(connection, socket);
    fetch_http_over_stream(&mut stream, host_header, path)
}

#[cfg(target_os = "scarlet")]
fn fetch_http_over_stream<S>(
    mut stream: S,
    host_header: &str,
    path: &str,
) -> Result<HttpResponse, String>
where
    S: Read + Write,
{
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nUser-Agent: carmine/0.1\r\nAccept: */*\r\n\r\n",
        path, host_header
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("write request: {}", e))?;

    read_http_response(&mut stream)
}

#[cfg(target_os = "scarlet")]
fn read_http_response<S>(stream: &mut S) -> Result<HttpResponse, String>
where
    S: Read,
{
    let mut response = Vec::new();
    let header_end = loop {
        if let Some(pos) = find_header_end(&response) {
            break pos;
        }

        let mut buf = [0u8; 1024];
        let n = stream
            .read(&mut buf)
            .map_err(|e| format!("read response headers: {}", e))?;
        if n == 0 {
            return Err("HTTP response ended before headers completed".to_string());
        }
        response.extend_from_slice(&buf[..n]);
    };

    let body_start = header_end + 4;
    let headers = String::from_utf8_lossy(&response[..header_end]).into_owned();
    let status_code = parse_status_code(&headers)?;
    let mut body = response[body_start..].to_vec();

    if header_value(&headers, "transfer-encoding")
        .map(|value| value.to_ascii_lowercase().contains("chunked"))
        .unwrap_or(false)
    {
        body = read_chunked_body(stream, body)?;
    } else if let Some(length) = header_value(&headers, "content-length") {
        let length = length
            .parse::<usize>()
            .map_err(|_| format!("invalid Content-Length: {}", length))?;
        while body.len() < length {
            let mut buf = [0u8; 1024];
            let n = stream
                .read(&mut buf)
                .map_err(|e| format!("read response body: {}", e))?;
            if n == 0 {
                return Err("HTTP response ended before Content-Length was satisfied".to_string());
            }
            body.extend_from_slice(&buf[..n]);
        }
        body.truncate(length);
    } else {
        stream
            .read_to_end(&mut body)
            .map_err(|e| format!("read response body: {}", e))?;
    }

    Ok(HttpResponse {
        status_code,
        headers,
        body,
    })
}

#[cfg(target_os = "scarlet")]
fn request_target(url: &url::Url) -> String {
    let mut path = if url.path().is_empty() {
        "/"
    } else {
        url.path()
    }
    .to_string();
    if let Some(query) = url.query() {
        path.push('?');
        path.push_str(query);
    }
    path
}

#[cfg(target_os = "scarlet")]
fn host_header(url: &url::Url, host: &str, port: u16) -> String {
    if url.port().is_some() {
        format!("{}:{}", host, port)
    } else {
        host.to_string()
    }
}

#[cfg(target_os = "scarlet")]
fn lookup_ipv4(host: &str) -> Result<Vec<[u8; 4]>, String> {
    if let Some(addr) = parse_ipv4(host) {
        return Ok(vec![addr]);
    }
    if !is_valid_hostname(host) {
        return Err("invalid hostname".to_string());
    }

    let socket = Socket::new().map_err(|e| format!("resolver socket create: {:?}", e))?;
    socket
        .connect(RESOLVERD_SOCKET_PATH)
        .map_err(|e| format!("resolverd unavailable: {:?}", e))?;
    let mut stream = ScarletStream::new(socket);

    let request = format!("A {}\n", host);
    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("resolver write: {}", e))?;

    let mut response = Vec::new();
    let mut buf = [0u8; 256];
    loop {
        let n = stream
            .read(&mut buf)
            .map_err(|e| format!("resolver read: {}", e))?;
        if n == 0 {
            break;
        }
        response.extend_from_slice(&buf[..n]);
        if response.contains(&b'\n') || response.len() > 4096 {
            break;
        }
    }

    parse_resolver_response(&response)
}

#[cfg(target_os = "scarlet")]
fn parse_resolver_response(response: &[u8]) -> Result<Vec<[u8; 4]>, String> {
    let text = std::str::from_utf8(response)
        .map_err(|_| "resolver response is not UTF-8".to_string())?
        .trim();

    let rest = text.strip_prefix("OK ").ok_or_else(|| {
        text.strip_prefix("ERR ")
            .unwrap_or("resolver error")
            .to_string()
    })?;
    let mut addrs = Vec::new();
    for item in rest.split_whitespace() {
        addrs.push(parse_ipv4(item).ok_or_else(|| "resolver returned invalid IPv4".to_string())?);
    }

    if addrs.is_empty() {
        Err("resolver returned no addresses".to_string())
    } else {
        Ok(addrs)
    }
}

#[cfg(target_os = "scarlet")]
fn parse_ipv4(value: &str) -> Option<[u8; 4]> {
    let mut parts = [0u8; 4];
    let mut index = 0;
    for part in value.split('.') {
        if index >= parts.len() {
            return None;
        }
        parts[index] = part.parse::<u8>().ok()?;
        index += 1;
    }
    if index == parts.len() {
        Some(parts)
    } else {
        None
    }
}

#[cfg(target_os = "scarlet")]
fn format_ipv4(addr: [u8; 4]) -> String {
    format!("{}.{}.{}.{}", addr[0], addr[1], addr[2], addr[3])
}

#[cfg(target_os = "scarlet")]
fn is_valid_hostname(host: &str) -> bool {
    if host.is_empty() || host.len() > 253 {
        return false;
    }

    for label in host.trim_end_matches('.').split('.') {
        if label.is_empty() || label.len() > 63 {
            return false;
        }
        let bytes = label.as_bytes();
        if bytes.first() == Some(&b'-') || bytes.last() == Some(&b'-') {
            return false;
        }
        if !bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || *b == b'-')
        {
            return false;
        }
    }

    true
}

#[cfg(target_os = "scarlet")]
fn find_header_end(response: &[u8]) -> Option<usize> {
    response.windows(4).position(|window| window == b"\r\n\r\n")
}

#[cfg(target_os = "scarlet")]
fn parse_status_code(headers: &str) -> Result<u16, String> {
    let status = headers
        .lines()
        .next()
        .ok_or_else(|| "HTTP response missing status line".to_string())?;
    status
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| format!("invalid HTTP status line: {}", status))?
        .parse::<u16>()
        .map_err(|_| format!("invalid HTTP status line: {}", status))
}

#[cfg(target_os = "scarlet")]
fn header_value(headers: &str, name: &str) -> Option<String> {
    for line in headers.lines().skip(1) {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if key.trim().eq_ignore_ascii_case(name) {
            return Some(value.trim().to_string());
        }
    }
    None
}

#[cfg(target_os = "scarlet")]
fn is_redirect(status_code: u16) -> bool {
    matches!(status_code, 301 | 302 | 303 | 307 | 308)
}

#[cfg(target_os = "scarlet")]
fn read_chunked_body<S>(stream: &mut S, mut body: Vec<u8>) -> Result<Vec<u8>, String>
where
    S: Read,
{
    loop {
        if let Some(decoded) = try_decode_chunked_body(&body)? {
            return Ok(decoded);
        }

        let mut buf = [0u8; 1024];
        let n = stream
            .read(&mut buf)
            .map_err(|e| format!("read chunked response body: {}", e))?;
        if n == 0 {
            return Err("chunked response ended before final chunk".to_string());
        }
        body.extend_from_slice(&buf[..n]);
    }
}

#[cfg(target_os = "scarlet")]
fn try_decode_chunked_body(body: &[u8]) -> Result<Option<Vec<u8>>, String> {
    let mut decoded = Vec::new();
    let mut cursor = 0;
    loop {
        let Some(line_end) = body[cursor..]
            .windows(2)
            .position(|window| window == b"\r\n")
        else {
            return Ok(None);
        };
        let line_end = line_end + cursor;
        let size_line = std::str::from_utf8(&body[cursor..line_end])
            .map_err(|_| "chunk size is not UTF-8".to_string())?;
        let size_text = size_line.split(';').next().unwrap_or("").trim();
        let size = usize::from_str_radix(size_text, 16)
            .map_err(|_| format!("invalid chunk size: {}", size_text))?;
        cursor = line_end + 2;
        if size == 0 {
            return Ok(Some(decoded));
        }
        let chunk_end = cursor
            .checked_add(size)
            .ok_or_else(|| "chunk size overflow".to_string())?;
        if chunk_end + 2 > body.len() {
            return Ok(None);
        }
        decoded.extend_from_slice(&body[cursor..chunk_end]);
        if &body[chunk_end..chunk_end + 2] != b"\r\n" {
            return Err("chunk missing trailing CRLF".to_string());
        }
        cursor = chunk_end + 2;
    }
}
