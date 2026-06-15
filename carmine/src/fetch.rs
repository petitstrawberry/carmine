use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

pub fn fetch_url(url: &str) -> Result<String, String> {
    let parsed = url::Url::parse(url).map_err(|e| format!("invalid URL: {}", e))?;

    if parsed.scheme() == "https" {
        return Err("HTTPS not supported yet".into());
    }

    let host = parsed.host_str().ok_or("no host in URL")?;
    let port = parsed.port_or_known_default().unwrap_or(80);
    let path = if parsed.path().is_empty() {
        "/"
    } else {
        parsed.path()
    };

    let addr = format!("{}:{}", host, port);
    let mut stream = TcpStream::connect(&addr).map_err(|e| format!("connect {}: {}", addr, e))?;

    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));

    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nUser-Agent: carmine/0.1\r\nAccept: text/html\r\n\r\n",
        path, host
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("write: {}", e))?;

    let mut response = Vec::with_capacity(8192);
    stream
        .read_to_end(&mut response)
        .map_err(|e| format!("read: {}", e))?;

    let response_str = String::from_utf8_lossy(&response);
    let body = response_str.split("\r\n\r\n").nth(1).unwrap_or("");
    Ok(body.to_string())
}
