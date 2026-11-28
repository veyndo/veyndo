//! Exit codes and user-facing failures: one stable code per failure class, so scripts can branch on them.
use std::fmt;
use veyndo_sdk::{Error, ProblemKind};

/// The documented exit codes (`docs/cli.md#exit-codes`). Clap's own usage errors exit with 2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
    /// 1: an unexpected failure (server internal error, undecodable response).
    Failure = 1,
    /// 2: the command line itself is wrong (also clap's code).
    Usage = 2,
    /// 3: not signed in, or the stored credential expired or was revoked.
    Authentication = 3,
    /// 4: signed in, but the credential lacks the right for this shop or operation.
    Permission = 4,
    /// 5: the shop, sandbox, product, order or app does not exist.
    NotFound = 5,
    /// 6: the input was refused: invalid file, CSV, manifest, plan or argument value.
    Invalid = 6,
    /// 7: a conflict with newer state (revision, digest, duplicate); review again and retry.
    Conflict = 7,
    /// 8: the server is unreachable, busy, rate limited or over quota; retry later.
    Unavailable = 8,
    /// 10: a check ran and reported blocking findings (`ops check`, `app validate`).
    Findings = 10,
}

/// A failure with its exit code and a message that never contains a secret.
#[derive(Debug)]
pub struct Failure {
    /// Exit code class.
    pub exit: Exit,
    /// Human-readable message.
    pub message: String,
}

/// Result type of every command.
pub type Outcome<T = ()> = Result<T, Failure>;

impl Failure {
    /// A failure of the given class.
    pub fn new(exit: Exit, message: impl Into<String>) -> Self {
        Self {
            exit,
            message: message.into(),
        }
    }
    /// The command line asks for something impossible.
    pub fn usage(message: impl Into<String>) -> Self {
        Self::new(Exit::Usage, message)
    }
    /// A local input was refused.
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(Exit::Invalid, message)
    }
    /// No usable credential.
    pub fn signed_out(message: impl Into<String>) -> Self {
        Self::new(Exit::Authentication, message)
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

/// The exit class of a server problem kind.
pub fn exit_of(kind: Option<&ProblemKind>, status: u16) -> Exit {
    match kind {
        Some(ProblemKind::AuthenticationRequired) => Exit::Authentication,
        Some(ProblemKind::PermissionDenied) => Exit::Permission,
        Some(ProblemKind::NotFound) => Exit::NotFound,
        Some(ProblemKind::ValidationError | ProblemKind::OutputContractError) => Exit::Invalid,
        Some(ProblemKind::Conflict | ProblemKind::RevisionConflict) => Exit::Conflict,
        Some(
            ProblemKind::QuotaExceeded
            | ProblemKind::RateLimited
            | ProblemKind::TemporarilyUnavailable
            | ProblemKind::ProviderUnavailable,
        ) => Exit::Unavailable,
        Some(ProblemKind::InternalError) => Exit::Failure,
        Some(ProblemKind::Unknown) | None => match status {
            401 => Exit::Authentication,
            403 => Exit::Permission,
            404 => Exit::NotFound,
            400 | 422 => Exit::Invalid,
            409 | 412 => Exit::Conflict,
            429 | 502..=504 => Exit::Unavailable,
            _ => Exit::Failure,
        },
    }
}

impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        match error {
            Error::Api(api) => {
                let exit = exit_of(api.kind(), api.status);
                let mut message = match api.problems.first() {
                    Some(problem) => {
                        let mut text = format!("{} ({})", problem.detail, problem.code);
                        if let Some(pointer) = &problem.pointer {
                            text.push_str(&format!(" at {pointer}"));
                        }
                        text
                    }
                    None => format!("HTTP {}", api.status),
                };
                if exit == Exit::Authentication {
                    message.push_str("; sign in again with `veyndo login`");
                }
                if api.retryable() {
                    message.push_str("; retrying the same request can succeed");
                }
                Self::new(exit, message)
            }
            Error::Mcp { code, message } => {
                Self::new(Exit::Invalid, format!("MCP error {code}: {message}"))
            }
            Error::Http(error) => Self::new(
                Exit::Unavailable,
                format!("Server unreachable: {}", without_query(&error.to_string())),
            ),
            Error::Decode(message) => Self::new(Exit::Failure, message),
        }
    }
}

/// Transport errors quote the URL; a query string never carries a secret here, but keep it out anyway.
fn without_query(text: &str) -> String {
    match text.split_once('?') {
        Some((head, _)) => format!("{head}…"),
        None => text.to_string(),
    }
}

impl From<std::io::Error> for Failure {
    fn from(error: std::io::Error) -> Self {
        Self::new(Exit::Failure, error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn problem_kinds_map_to_stable_exit_codes() {
        assert_eq!(
            exit_of(Some(&ProblemKind::AuthenticationRequired), 401),
            Exit::Authentication
        );
        assert_eq!(
            exit_of(Some(&ProblemKind::PermissionDenied), 403),
            Exit::Permission
        );
        assert_eq!(
            exit_of(Some(&ProblemKind::RevisionConflict), 409),
            Exit::Conflict
        );
        assert_eq!(
            exit_of(Some(&ProblemKind::RateLimited), 429),
            Exit::Unavailable
        );
        assert_eq!(exit_of(None, 404), Exit::NotFound);
        assert_eq!(exit_of(Some(&ProblemKind::Unknown), 503), Exit::Unavailable);
        assert_eq!(exit_of(None, 500), Exit::Failure);
        assert_eq!(Exit::Findings as i32, 10);
    }

    #[test]
    fn transport_messages_drop_queries() {
        assert_eq!(
            without_query("error sending http://x/a?b=c"),
            "error sending http://x/a…"
        );
    }
}
