//! Loopback-only HTTP server used for first-run setup and the viewer settings.
//!
//! The Frigate UI itself is loaded directly from the user's server; this server
//! never proxies Frigate traffic. All pages are static, have no JavaScript, and
//! live under a random per-launch path token.

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::config::{is_reachable, normalize_url, Config, ConfigStore};
use crate::pages::{self, PageData, PageMode};

const ICON_PNG: &[u8] = include_bytes!("../assets/frigate-mark.png");
const MAX_HEADER_BYTES: usize = 64 * 1024;
const MAX_BODY_BYTES: usize = 16 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

const SECURITY_HEADERS: &str = concat!(
    "X-Content-Type-Options: nosniff\r\n",
    "Referrer-Policy: no-referrer\r\n",
    "X-Frame-Options: DENY\r\n",
    "Content-Security-Policy: default-src 'none'; img-src 'self'; style-src 'unsafe-inline'; frame-ancestors 'none'\r\n",
);

pub struct LocalServer {
    base_url: String,
    settings_url: String,
}

impl LocalServer {
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn settings_url(&self) -> &str {
        &self.settings_url
    }
}

struct ServerState {
    store: Arc<ConfigStore>,
    token: String,
    base_url: String,
}

pub fn start(store: Arc<ConfigStore>) -> io::Result<LocalServer> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    let token = random_token();
    let base_url = format!("http://127.0.0.1:{port}/{token}/");
    let settings_url = format!("{base_url}settings");

    let state = Arc::new(ServerState {
        store,
        token: token.clone(),
        base_url: base_url.clone(),
    });

    thread::Builder::new()
        .name("frigate-setup".to_string())
        .spawn(move || {
            for connection in listener.incoming() {
                match connection {
                    Ok(stream) => {
                        let state = Arc::clone(&state);
                        let _ = thread::Builder::new()
                            .name("frigate-setup-conn".to_string())
                            .spawn(move || {
                                if let Err(err) = handle_connection(stream, &state) {
                                    log::debug!("setup request ended with error: {err}");
                                }
                            });
                    }
                    Err(err) => log::warn!("setup server accept failed: {err}"),
                }
            }
        })?;

    log::info!("local setup server listening on {base_url}");
    Ok(LocalServer {
        base_url,
        settings_url,
    })
}

struct Request {
    method: String,
    path: String,
    query: HashMap<String, String>,
    body: Vec<u8>,
}

fn handle_connection(mut stream: TcpStream, state: &ServerState) -> io::Result<()> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));

    let request = match read_request(&mut stream) {
        Ok(Some(request)) => request,
        Ok(None) => return Ok(()),
        Err(err) => {
            log::debug!("bad setup request: {err}");
            return respond_text(&mut stream, 400, "Bad request");
        }
    };

    let segments: Vec<&str> = request.path.split('/').filter(|part| !part.is_empty()).collect();
    if segments.first().copied() != Some(state.token.as_str()) {
        return respond_text(&mut stream, 404, "Not found");
    }
    let route = segments.get(1..).map(|parts| parts.join("/")).unwrap_or_default();

    match (request.method.as_str(), route.as_str()) {
        ("GET", "") => {
            let config = state.store.get();
            let mode = if config.is_some() {
                PageMode::Settings
            } else {
                PageMode::Setup
            };
            let url = config
                .as_ref()
                .map(|config| config.server_url.clone())
                .unwrap_or_default();
            let allow_invalid = config
                .as_ref()
                .map(|config| config.allow_invalid_certificates)
                .unwrap_or(false);
            let keep_screen_on = config
                .as_ref()
                .map(|config| config.keep_screen_on)
                .unwrap_or(true);

            render_page(
                &mut stream,
                200,
                state,
                mode,
                url,
                allow_invalid,
                keep_screen_on,
                None,
                false,
                request.query.contains_key("offline"),
            )
        }
        ("GET", "settings") => {
            let config = state.store.get();
            let mode = if config.is_some() {
                PageMode::Settings
            } else {
                PageMode::Setup
            };
            let url = config
                .as_ref()
                .map(|config| config.server_url.clone())
                .unwrap_or_default();
            let allow_invalid = config
                .as_ref()
                .map(|config| config.allow_invalid_certificates)
                .unwrap_or(false);
            let keep_screen_on = config
                .as_ref()
                .map(|config| config.keep_screen_on)
                .unwrap_or(true);

            render_page(
                &mut stream,
                200,
                state,
                mode,
                url,
                allow_invalid,
                keep_screen_on,
                None,
                false,
                false,
            )
        }
        ("GET", "icon.png") | ("GET", "favicon.ico") => {
            respond_bytes(&mut stream, 200, "image/png", ICON_PNG, true)
        }
        ("POST", "save") => save_settings(&mut stream, state, &request),
        ("POST", "forget") => {
            if let Err(err) = state.store.clear() {
                log::warn!("could not clear config: {err}");
                return respond_text(&mut stream, 500, "Could not clear the saved server.");
            }
            respond_redirect(&mut stream, state_base_url(state))
        }
        ("GET", "reload") => match state.store.get() {
            Some(config) => respond_redirect(&mut stream, &config.server_url),
            None => respond_redirect(&mut stream, state_base_url(state)),
        },
        _ => respond_text(&mut stream, 404, "Not found"),
    }
}

fn state_base_url(state: &ServerState) -> &str {
    &state.base_url
}

#[allow(clippy::too_many_arguments)]
fn save_settings(
    stream: &mut TcpStream,
    state: &ServerState,
    request: &Request,
) -> io::Result<()> {
    let form = parse_pairs(&String::from_utf8_lossy(&request.body));
    let submitted_url = form.get("url").cloned().unwrap_or_default();
    let allow_invalid = form.contains_key("allow_invalid");
    let keep_screen_on = form.contains_key("keep_screen_on");
    let force = form.get("force").map(|value| value == "1").unwrap_or(false);

    let config = state.store.get();
    let mode = if config.is_some() {
        PageMode::Settings
    } else {
        PageMode::Setup
    };

    let normalized = match normalize_url(&submitted_url) {
        Ok(url) => url,
        Err(error) => {
            return render_page(
                stream,
                400,
                state,
                mode,
                submitted_url,
                allow_invalid,
                keep_screen_on,
                Some(error),
                false,
                false,
            );
        }
    };

    if !force {
        if let Err(error) = is_reachable(&normalized, CONNECT_TIMEOUT) {
            let message = format!(
                "{error} Check the address and network, or choose “Save anyway” if you know it is correct."
            );
            return render_page(
                stream,
                200,
                state,
                mode,
                submitted_url,
                allow_invalid,
                keep_screen_on,
                Some(message),
                true,
                false,
            );
        }
    }

    let config = Config {
        server_url: normalized.clone(),
        allow_invalid_certificates: allow_invalid,
        keep_screen_on,
    };

    match state.store.save(&config) {
        Ok(()) => respond_redirect(stream, &normalized),
        Err(err) => {
            log::error!("could not save config: {err}");
            render_page(
                stream,
                500,
                state,
                mode,
                submitted_url,
                allow_invalid,
                keep_screen_on,
                Some(format!("Could not save the server settings: {err}")),
                false,
                false,
            )
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_page(
    stream: &mut TcpStream,
    status: u16,
    state: &ServerState,
    mode: PageMode,
    url: String,
    allow_invalid: bool,
    keep_screen_on: bool,
    error: Option<String>,
    force_available: bool,
    offline: bool,
) -> io::Result<()> {
    let current_server = state.store.get().map(|config| config.server_url);
    let html = pages::render(&PageData {
        mode,
        base_path: format!("/{}/", state.token),
        url,
        allow_invalid_certificates: allow_invalid,
        keep_screen_on,
        error,
        force_available,
        offline,
        current_server,
    });

    respond_bytes(
        stream,
        status,
        "text/html; charset=utf-8",
        html.as_bytes(),
        true,
    )
}

fn read_request(stream: &mut TcpStream) -> Result<Option<Request>, String> {
    let mut buffer = Vec::with_capacity(4096);
    let mut chunk = [0u8; 4096];

    let header_end = loop {
        if let Some(position) = find_bytes(&buffer, b"\r\n\r\n") {
            break position;
        }
        if buffer.len() > MAX_HEADER_BYTES {
            return Err("request headers are too large".to_string());
        }
        let read = stream
            .read(&mut chunk)
            .map_err(|err| format!("read failed: {err}"))?;
        if read == 0 {
            if buffer.is_empty() {
                return Ok(None);
            }
            return Err("connection closed in the headers".to_string());
        }
        buffer.extend_from_slice(&chunk[..read]);
    };

    let head = std::str::from_utf8(&buffer[..header_end])
        .map_err(|_| "request headers are not UTF-8".to_string())?;
    let mut lines = head.split("\r\n");
    let request_line = lines.next().ok_or_else(|| "missing request line".to_string())?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let target = parts.next().unwrap_or_default().to_string();
    let version = parts.next().unwrap_or_default();
    if method.is_empty() || target.is_empty() || !version.starts_with("HTTP/") {
        return Err("malformed request line".to_string());
    }

    let mut content_length = 0usize;
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| "invalid Content-Length".to_string())?;
            }
        }
    }
    if content_length > MAX_BODY_BYTES {
        return Err("request body is too large".to_string());
    }

    let body_start = header_end + 4;
    let mut body = buffer[body_start..].to_vec();
    while body.len() < content_length {
        let read = stream
            .read(&mut chunk)
            .map_err(|err| format!("read failed: {err}"))?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
        if body.len() > MAX_BODY_BYTES {
            return Err("request body is too large".to_string());
        }
    }
    body.truncate(content_length);

    let (path, query_text) = target.split_once('?').unwrap_or((target.as_str(), ""));
    let path = percent_decode(path, false);
    if !path.starts_with('/') {
        return Err("request path must be absolute".to_string());
    }
    let query = parse_pairs(query_text);

    Ok(Some(Request {
        method,
        path,
        query,
        body,
    }))
}

fn respond_text(stream: &mut TcpStream, status: u16, text: &str) -> io::Result<()> {
    respond_bytes(
        stream,
        status,
        "text/plain; charset=utf-8",
        text.as_bytes(),
        true,
    )
}

fn respond_bytes(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
    security_headers: bool,
) -> io::Result<()> {
    let reason = reason_phrase(status);
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\n\
         Server: FrigateViewer\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Cache-Control: no-store, max-age=0\r\n\
         Pragma: no-cache\r\n\
         Connection: close\r\n",
        body.len()
    )?;
    if security_headers {
        stream.write_all(SECURITY_HEADERS.as_bytes())?;
    }
    stream.write_all(b"\r\n")?;
    stream.write_all(body)?;
    stream.flush()
}

fn respond_redirect(stream: &mut TcpStream, location: &str) -> io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 303 See Other\r\n\
         Server: FrigateViewer\r\n\
         Location: {location}\r\n\
         Content-Length: 0\r\n\
         Cache-Control: no-store\r\n\
         Connection: close\r\n\r\n"
    )?;
    stream.flush()
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        303 => "See Other",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        500 => "Internal Server Error",
        _ => "Response",
    }
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn parse_pairs(input: &str) -> HashMap<String, String> {
    let mut pairs = HashMap::new();
    for part in input.split('&') {
        if part.is_empty() {
            continue;
        }
        let (key, value) = part.split_once('=').unwrap_or((part, ""));
        let key = percent_decode(key, true);
        if key.is_empty() {
            continue;
        }
        let value = percent_decode(value, true);
        pairs.insert(key, value);
    }
    pairs
}

fn percent_decode(input: &str, plus_as_space: bool) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                let high = hex_value(bytes[index + 1]);
                let low = hex_value(bytes[index + 2]);
                if let (Some(high), Some(low)) = (high, low) {
                    out.push(high * 16 + low);
                    index += 3;
                } else {
                    out.push(bytes[index]);
                    index += 1;
                }
            }
            b'+' if plus_as_space => {
                out.push(b' ');
                index += 1;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn random_token() -> String {
    let mut bytes = [0u8; 16];
    if let Ok(mut file) = std::fs::File::open("/dev/urandom") {
        if file.read_exact(&mut bytes).is_ok() {
            return hex_encode(&bytes);
        }
    }

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0x9e37_79b9_7f4a_7c15)
        ^ (std::process::id() as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    let mut state = nanos;
    for byte in &mut bytes {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        *byte = state as u8;
    }
    hex_encode(&bytes)
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_form_pairs() {
        let pairs = parse_pairs("url=http%3A%2F%2Ffrigate%3A5000%2F&keep_screen_on=1&empty=");
        assert_eq!(pairs.get("url").unwrap(), "http://frigate:5000/");
        assert!(pairs.contains_key("keep_screen_on"));
        assert_eq!(pairs.get("empty").unwrap(), "");
    }

    #[test]
    fn decodes_plus_as_space() {
        let pairs = parse_pairs("message=hello+world");
        assert_eq!(pairs.get("message").unwrap(), "hello world");
    }

    #[test]
    fn token_is_hex_and_long_enough() {
        let token = random_token();
        assert_eq!(token.len(), 32);
        assert!(token.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }

    #[test]
    fn finds_header_boundary() {
        assert_eq!(find_bytes(b"abc\r\n\r\ndef", b"\r\n\r\n"), Some(3));
        assert_eq!(find_bytes(b"abc", b"\r\n\r\n"), None);
    }
}
