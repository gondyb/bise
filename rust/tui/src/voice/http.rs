//! A small blocking HTTP/1.1 client for the voice request (BISE-130):
//! one POST, `Connection: close`, rustls with the webpki roots (the
//! trust store the old realtime websocket used). The body is in memory;
//! nothing is written to disk.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

#[derive(Clone, PartialEq, Eq)]
pub struct Request {
    pub url: String,
    /// never printed: it holds the key
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl std::fmt::Debug for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Request {{ url: {:?}, body: <{} bytes> }}", self.url, self.body.len())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
}

/// scheme, host, port, path+query
pub fn split_url(url: &str) -> Result<(bool, String, u16, String), String> {
    let (tls, rest) = if let Some(r) = url.strip_prefix("https://") {
        (true, r)
    } else if let Some(r) = url.strip_prefix("http://") {
        (false, r)
    } else {
        return Err(format!("not an http(s) URL: {}", url));
    };
    let (hostport, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, port) = match hostport.rsplit_once(':') {
        Some((h, p)) if !h.ends_with(']') || h.starts_with('[') => {
            (h.to_string(), p.parse::<u16>().map_err(|_| format!("bad port in {}", url))?)
        }
        _ => (hostport.to_string(), if tls { 443 } else { 80 }),
    };
    if host.is_empty() {
        return Err(format!("no host in {}", url));
    }
    Ok((tls, host.trim_matches(|c| c == '[' || c == ']').to_string(), port, path.to_string()))
}

fn tls_config() -> Arc<rustls::ClientConfig> {
    static CONFIG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let roots = rustls::RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() };
            let provider = Arc::new(rustls::crypto::ring::default_provider());
            let config = rustls::ClientConfig::builder_with_provider(provider)
                .with_safe_default_protocol_versions()
                .expect("rustls: the default protocol versions")
                .with_root_certificates(roots)
                .with_no_client_auth();
            Arc::new(config)
        })
        .clone()
}

/// POST `req`; `timeout` bounds the connect and each read.
pub fn send(req: &Request, timeout: Duration) -> Result<Response, String> {
    let (tls, host, port, path) = split_url(&req.url)?;
    let addrs: Vec<_> = (host.as_str(), port)
        .to_socket_addrs()
        .map_err(|e| format!("cannot resolve {}: {}", host, e))?
        .collect();
    let connect = Duration::from_secs(10).min(timeout);
    let mut last = format!("cannot resolve {}", host);
    let mut tcp = None;
    for a in &addrs {
        match TcpStream::connect_timeout(a, connect) {
            Ok(s) => {
                tcp = Some(s);
                break;
            }
            Err(e) => last = format!("cannot connect to {}: {}", host, e),
        }
    }
    let tcp = tcp.ok_or(last)?;
    let _ = tcp.set_read_timeout(Some(timeout));
    let _ = tcp.set_write_timeout(Some(timeout));
    let _ = tcp.set_nodelay(true);
    let mut head = format!(
        "POST {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: bise\r\nAccept: application/json\r\nConnection: close\r\nContent-Length: {}\r\n",
        path,
        host,
        req.body.len()
    );
    for (k, v) in &req.headers {
        head.push_str(&format!("{}: {}\r\n", k, v));
    }
    head.push_str("\r\n");
    if tls {
        let name = rustls::pki_types::ServerName::try_from(host.clone())
            .map_err(|_| format!("bad host name {}", host))?;
        let conn = rustls::ClientConnection::new(tls_config(), name).map_err(|e| e.to_string())?;
        let mut s = rustls::StreamOwned::new(conn, tcp);
        exchange(&mut s, head.as_bytes(), &req.body)
    } else {
        let mut s = tcp;
        exchange(&mut s, head.as_bytes(), &req.body)
    }
}

fn io_error(e: std::io::Error) -> String {
    match e.kind() {
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut => "the request timed out".into(),
        _ => e.to_string(),
    }
}

fn exchange<S: Read + Write>(s: &mut S, head: &[u8], body: &[u8]) -> Result<Response, String> {
    s.write_all(head).map_err(io_error)?;
    s.write_all(body).map_err(io_error)?;
    s.flush().map_err(io_error)?;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 16 * 1024];
    loop {
        if let Some(r) = parse_response(&buf, false)? {
            return Ok(r);
        }
        match s.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            // a TLS peer that closes without close_notify: the end
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(io_error(e)),
        }
    }
    parse_response(&buf, true)?.ok_or_else(|| "the connection closed before the response ended".into())
}

/// A whole response in `buf`, or None while more bytes are needed.
/// `eof`: the connection closed (a body without length ends there).
pub fn parse_response(buf: &[u8], eof: bool) -> Result<Option<Response>, String> {
    let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") else {
        return Ok(None);
    };
    let head = String::from_utf8_lossy(&buf[..end]);
    let mut lines = head.split("\r\n");
    let status_line = lines.next().unwrap_or("");
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or_else(|| format!("not an HTTP response: {}", super::stt::one_line(status_line)))?;
    let (mut length, mut chunked) = (None, false);
    for l in lines {
        let Some((k, v)) = l.split_once(':') else { continue };
        let (k, v) = (k.trim().to_ascii_lowercase(), v.trim());
        if k == "content-length" {
            length = v.parse::<usize>().ok();
        } else if k == "transfer-encoding" && v.to_ascii_lowercase().contains("chunked") {
            chunked = true;
        }
    }
    let rest = &buf[end + 4..];
    if (100..200).contains(&status) {
        // an interim response (100 Continue): the real one follows
        return parse_response(rest, eof);
    }
    let body = if chunked {
        dechunk(rest)
    } else if let Some(n) = length {
        (rest.len() >= n).then(|| rest[..n].to_vec())
    } else {
        eof.then(|| rest.to_vec())
    };
    Ok(body.map(|body| Response { status, body }))
}

/// A chunked body, once its last chunk arrived.
fn dechunk(mut b: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let eol = b.windows(2).position(|w| w == b"\r\n")?;
        let size = String::from_utf8_lossy(&b[..eol]);
        let size = usize::from_str_radix(size.split(';').next().unwrap_or("").trim(), 16).ok()?;
        b = &b[eol + 2..];
        if size == 0 {
            return Some(out);
        }
        if b.len() < size + 2 {
            return None;
        }
        out.extend_from_slice(&b[..size]);
        b = &b[size + 2..];
    }
}
