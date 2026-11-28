//! The signed-in profile: server, selected shop and credential in one owner-only (0600) file; environment overrides for CI.
use crate::failure::{Exit, Failure, Outcome};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use veyndo_sdk::{Auth, Client, ClientOptions};

/// The production API origin used when nothing else is configured.
pub const DEFAULT_SERVER: &str = "https://api.veyndo.ai";

/// How the stored secret authenticates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// A personal merchant session from `POST /api/auth/login` (expires after 12 hours).
    Session,
    /// A scoped integration key bound to one shop.
    IntegrationKey,
}

/// The stored credential. `Debug` never prints the secret.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credential {
    /// Credential type.
    pub kind: Kind,
    /// Bearer secret; only ever sent as `Authorization: Bearer`.
    pub secret: String,
    /// Account email of a personal session, for `veyndo status`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

impl std::fmt::Debug for Credential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credential")
            .field("kind", &self.kind)
            .field("secret", &"<redacted>")
            .finish()
    }
}

/// Contents of `credentials.json`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    /// Server origin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    /// Selected shop (sent as `x-tenant`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shop: Option<String>,
    /// Stored credential.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential: Option<Credential>,
}

/// Where the profile lives: `VEYNDO_CONFIG_DIR`, else `$XDG_CONFIG_HOME/veyndo`, else `~/.config/veyndo`.
pub fn directory() -> Outcome<PathBuf> {
    if let Some(dir) = std::env::var_os("VEYNDO_CONFIG_DIR") {
        return Ok(PathBuf::from(dir));
    }
    if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME").filter(|d| !d.is_empty()) {
        return Ok(PathBuf::from(dir).join("veyndo"));
    }
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".config/veyndo"))
        .ok_or_else(|| Failure::new(Exit::Failure, "Set HOME or VEYNDO_CONFIG_DIR"))
}

fn file() -> Outcome<PathBuf> {
    Ok(directory()?.join("credentials.json"))
}

/// Reads the stored profile; a missing file is an empty profile.
pub fn load() -> Outcome<Profile> {
    let path = file()?;
    match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).map_err(|_| {
            Failure::invalid(format!(
                "{} is not a valid profile; remove it and sign in again",
                path.display()
            ))
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Profile::default()),
        Err(error) => Err(error.into()),
    }
}

/// Writes the profile atomically with owner-only permissions.
pub fn save(profile: &Profile) -> Outcome<PathBuf> {
    let path = file()?;
    let dir = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(dir)?;
    restrict(dir, 0o700)?;
    let temporary = dir.join(".credentials.json.tmp");
    {
        let mut out = open_private(&temporary)?;
        out.write_all(
            serde_json::to_string_pretty(profile)
                .unwrap_or_default()
                .as_bytes(),
        )?;
        out.write_all(b"\n")?;
        out.sync_all()?;
    }
    fs::rename(&temporary, &path)?;
    restrict(&path, 0o600)?;
    Ok(path)
}

/// Removes the stored profile.
pub fn remove() -> Outcome<bool> {
    match fs::remove_file(file()?) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

#[cfg(unix)]
fn open_private(path: &Path) -> std::io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn open_private(path: &Path) -> std::io::Result<fs::File> {
    fs::File::create(path)
}

#[cfg(unix)]
fn restrict(path: &Path, mode: u32) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}

#[cfg(not(unix))]
fn restrict(_: &Path, _: u32) -> std::io::Result<()> {
    Ok(())
}

/// Global options that select server, shop and credential for one invocation.
#[derive(Clone, Debug, Default)]
pub struct Selection {
    /// `--server` / `VEYNDO_SERVER`.
    pub server: Option<String>,
    /// `--shop` / `VEYNDO_SHOP`.
    pub shop: Option<String>,
}

/// The effective server, shop and credential: flags and environment before the stored profile.
#[derive(Clone, Debug)]
pub struct Context {
    /// Server origin without a trailing slash.
    pub server: String,
    /// Selected shop.
    pub shop: Option<String>,
    /// Credential, if any.
    pub credential: Option<Credential>,
    /// Whether the credential came from `VEYNDO_API_KEY` rather than the profile.
    pub from_environment: bool,
}

impl Context {
    /// Resolves flags, `VEYNDO_API_KEY` and the stored profile.
    pub fn resolve(selection: &Selection) -> Outcome<Self> {
        let stored = load()?;
        let environment = std::env::var("VEYNDO_API_KEY")
            .ok()
            .filter(|key| !key.trim().is_empty())
            .map(|secret| Credential {
                kind: Kind::IntegrationKey,
                secret: secret.trim().to_string(),
                email: None,
            });
        let from_environment = environment.is_some();
        let server = selection
            .server
            .clone()
            .or(stored.server)
            .unwrap_or_else(|| DEFAULT_SERVER.into());
        Ok(Self {
            server: normalize_server(&server)?,
            shop: selection.shop.clone().or(stored.shop),
            credential: environment.or(stored.credential),
            from_environment,
        })
    }
    /// An anonymous client for public reads.
    pub fn anonymous(&self) -> Client {
        Client::new(ClientOptions {
            base_url: self.server.clone(),
            headers: vec![("user-agent".into(), user_agent())],
            ..ClientOptions::default()
        })
    }
    /// A client with the credential and the selected shop; fails when either is missing.
    pub fn client(&self) -> Outcome<Client> {
        let shop = self.require_shop()?;
        self.client_for(&shop)
    }
    /// A client with the credential for an explicit shop or sandbox.
    pub fn client_for(&self, shop: &str) -> Outcome<Client> {
        let credential = self.credential.as_ref().ok_or_else(|| {
            Failure::signed_out("Not signed in; run `veyndo login` or set VEYNDO_API_KEY")
        })?;
        let auth = match credential.kind {
            Kind::Session => Auth::PersonalSession(credential.secret.clone()),
            Kind::IntegrationKey => Auth::IntegrationKey(credential.secret.clone()),
        };
        let shop = shop.to_string();
        Ok(self.anonymous().with(|o| {
            o.auth = Some(auth);
            o.tenant = Some(shop);
        }))
    }
    /// The selected shop.
    pub fn require_shop(&self) -> Outcome<String> {
        self.shop.clone().ok_or_else(|| {
            Failure::usage(
                "No shop selected; pass --shop, set VEYNDO_SHOP or run `veyndo shop use <id>`",
            )
        })
    }
}

/// `veyndo/<version>` so server logs can tell CLI traffic apart.
pub fn user_agent() -> String {
    format!("veyndo-cli/{}", env!("CARGO_PKG_VERSION"))
}

/// Accepts `https://host` or `http://localhost:port`; plain HTTP only to a loopback address.
pub fn normalize_server(server: &str) -> Outcome<String> {
    let server = server.trim().trim_end_matches('/');
    let (scheme, rest) = server.split_once("://").ok_or_else(|| {
        Failure::usage(format!("Server must be an http(s) origin, got {server:?}"))
    })?;
    let host = rest.split(['/', ':']).next().unwrap_or_default();
    let loopback =
        matches!(host, "localhost" | "127.0.0.1" | "[::1]") || host.ends_with(".localhost");
    match scheme {
        "https" if !host.is_empty() => Ok(server.to_string()),
        "http" if loopback => Ok(server.to_string()),
        "http" => Err(Failure::usage(
            "Plain HTTP is only allowed for a loopback server; use https://",
        )),
        _ => Err(Failure::usage(format!(
            "Server must be an http(s) origin, got {server:?}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn servers_need_https_except_loopback() {
        assert_eq!(
            normalize_server("https://api.veyndo.ai/").unwrap(),
            "https://api.veyndo.ai"
        );
        assert!(normalize_server("http://127.0.0.1:8080").is_ok());
        assert!(normalize_server("http://shop.localhost:3000").is_ok());
        assert!(normalize_server("http://api.veyndo.ai").is_err());
        assert!(normalize_server("api.veyndo.ai").is_err());
        assert!(normalize_server("ftp://x").is_err());
    }

    #[test]
    fn debug_output_redacts_the_secret() {
        let credential = Credential {
            kind: Kind::IntegrationKey,
            secret: "vk_live_very_secret".into(),
            email: None,
        };
        let shown = format!("{credential:?}");
        assert!(!shown.contains("very_secret"));
        assert!(shown.contains("redacted"));
    }

    #[cfg(unix)]
    #[test]
    fn the_profile_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("veyndo-cli-test-{}", std::process::id()));
        let path = dir.join("credentials.json");
        fs::create_dir_all(&dir).unwrap();
        {
            let mut out = open_private(&path).unwrap();
            out.write_all(b"{}").unwrap();
        }
        restrict(&path, 0o600).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        fs::remove_dir_all(&dir).unwrap();
    }
}
