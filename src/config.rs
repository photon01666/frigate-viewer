use std::io;
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use url::Url;

/// Current runtime copies of security/behaviour settings.
///
/// The Java side cannot call into a Mutex-protected Rust struct each time it
/// needs a decision, so the two settings it cares about are mirrored here.
static ALLOW_INVALID_CERTIFICATES: AtomicBool = AtomicBool::new(false);
static KEEP_SCREEN_ON: AtomicBool = AtomicBool::new(true);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    pub server_url: String,
    #[serde(default)]
    pub allow_invalid_certificates: bool,
    #[serde(default = "default_keep_screen_on")]
    pub keep_screen_on: bool,
}

fn default_keep_screen_on() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server_url: String::new(),
            allow_invalid_certificates: false,
            keep_screen_on: true,
        }
    }
}

/// A tiny JSON config store in the app's private data directory.
pub struct ConfigStore {
    path: PathBuf,
    config: Mutex<Option<Config>>,
}

impl ConfigStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let config = match Self::read(&path) {
            Ok(config) => config,
            Err(err) => {
                log::warn!("could not read {}: {err}", path.display());
                None
            }
        };
        set_runtime_flags(config.as_ref());
        Self {
            path,
            config: Mutex::new(config),
        }
    }

    fn read(path: &Path) -> io::Result<Option<Config>> {
        match std::fs::read_to_string(path) {
            Ok(text) => match serde_json::from_str::<Config>(&text) {
                Ok(config) => {
                    if config.server_url.trim().is_empty() {
                        Ok(None)
                    } else {
                        Ok(Some(config))
                    }
                }
                Err(err) => {
                    log::warn!("ignoring invalid config: {err}");
                    Ok(None)
                }
            },
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err),
        }
    }

    pub fn get(&self) -> Option<Config> {
        self.lock().clone()
    }

    pub fn save(&self, config: &Config) -> io::Result<()> {
        let json = serde_json::to_vec_pretty(config)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, &self.path)?;

        set_runtime_flags(Some(config));
        *self.lock() = Some(config.clone());
        Ok(())
    }

    pub fn clear(&self) -> io::Result<()> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }
        set_runtime_flags(None);
        *self.lock() = None;
        Ok(())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<Config>> {
        self.config.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn set_runtime_flags(config: Option<&Config>) {
    let allow_invalid = config
        .map(|config| config.allow_invalid_certificates)
        .unwrap_or(false);
    let keep_screen_on = config
        .map(|config| config.keep_screen_on)
        .unwrap_or(true);

    ALLOW_INVALID_CERTIFICATES.store(allow_invalid, Ordering::Relaxed);
    KEEP_SCREEN_ON.store(keep_screen_on, Ordering::Relaxed);
}

pub fn allow_invalid_certificates() -> bool {
    ALLOW_INVALID_CERTIFICATES.load(Ordering::Relaxed)
}

pub fn keep_screen_on() -> bool {
    KEEP_SCREEN_ON.load(Ordering::Relaxed)
}

/// Add `http://` when the user omitted it, reject unsupported schemes and
/// strip fragments/queries that do not make sense for a Frigate base URL.
pub fn normalize_url(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Enter your Frigate server address.".to_string());
    }

    let candidate = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    };

    let mut url = Url::parse(&candidate)
        .map_err(|_| "That does not look like a complete server address.".to_string())?;

    match url.scheme() {
        "http" | "https" => {}
        _ => return Err("Use an http:// or https:// address.".to_string()),
    }

    if url.host_str().is_none() {
        return Err("The server address is missing a host name.".to_string());
    }

    if !url.username().is_empty() || url.password().is_some() {
        return Err(
            "Leave the username and password out of the URL and sign in on Frigate's login page."
                .to_string(),
        );
    }

    url.set_fragment(None);
    url.set_query(None);
    Ok(url.to_string())
}

/// Fast TCP reachability check, used to choose between the setup page and the
/// Frigate UI at startup and to warn about typos when saving a new address.
pub fn is_reachable(server_url: &str, timeout: Duration) -> Result<(), String> {
    let url = Url::parse(server_url)
        .map_err(|_| "That server address is not a valid URL.".to_string())?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("Only http:// and https:// addresses are supported.".to_string());
    }

    let port = url
        .port_or_known_default()
        .ok_or_else(|| "The URL needs a port (for example :5000).".to_string())?;

    let addresses: Vec<SocketAddr> = url
        .socket_addrs(|| Some(port))
        .map_err(|err| format!("Could not look up the server: {err}"))?;

    if addresses.is_empty() {
        return Err("Could not resolve the server address.".to_string());
    }

    let mut last_error = None;
    for address in addresses {
        match TcpStream::connect_timeout(&address, timeout) {
            Ok(stream) => {
                let _ = stream.set_nodelay(true);
                return Ok(());
            }
            Err(err) => last_error = Some(err),
        }
    }

    let host = url.host_str().unwrap_or("the server");
    Err(match last_error {
        Some(err) => format!("Could not connect to {host}:{port} ({err})."),
        None => format!("Could not connect to {host}:{port}."),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_http_scheme() {
        assert_eq!(
            normalize_url("192.168.1.20:5000").unwrap(),
            "http://192.168.1.20:5000/"
        );
        assert_eq!(
            normalize_url("frigate.local").unwrap(),
            "http://frigate.local/"
        );
    }

    #[test]
    fn keeps_https_and_paths() {
        assert_eq!(
            normalize_url("https://nvr.example.com:8971").unwrap(),
            "https://nvr.example.com:8971/"
        );
        assert_eq!(
            normalize_url("https://example.com/frigate/").unwrap(),
            "https://example.com/frigate/"
        );
    }

    #[test]
    fn drops_queries_and_fragments() {
        assert_eq!(
            normalize_url("http://192.168.1.2:5000/?foo=bar#baz").unwrap(),
            "http://192.168.1.2:5000/"
        );
    }

    #[test]
    fn rejects_unsupported_schemes() {
        assert!(normalize_url("ftp://example.com").is_err());
        assert!(normalize_url("javascript:alert(1)").is_err());
    }

    #[test]
    fn rejects_embedded_credentials() {
        assert!(normalize_url("http://user:pass@example.com:5000").is_err());
    }
}
