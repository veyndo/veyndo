//! Shared foundation of every component: the error contract, its conversions and basic identifiers. No business rules.
pub mod access;
mod conversions;
mod failure;
pub mod http_limits;
pub mod observability;
pub mod request_context;
pub mod shop_scope;
pub mod work_signal;

pub use axum::http::StatusCode;
pub use failure::{Error, Failure};
pub use request_context::RequestContext;
use sha2::{Digest, Sha256};

pub type Result<T> = std::result::Result<T, Error>;

pub fn bad(s: impl Into<String>) -> Error {
    Error(StatusCode::BAD_REQUEST, s.into())
}
pub fn conflict(s: &str) -> Error {
    Error(StatusCode::CONFLICT, s.into())
}
pub fn uid() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}
pub fn hash(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))
}
pub fn validate_tenant(t: &str) -> Result<()> {
    if !veyndo_kernel::verified_kernel::workspace_length_admissible(t.len() as u64)
        || !t
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        || t.starts_with('-')
        || t.ends_with('-')
    {
        return Err(bad(
            "Workspace ID must be 2..48 lowercase letters, numbers or hyphens",
        ));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn workspace_ids_are_short_lowercase_names_without_edge_hyphens() {
        for valid in ["ab", "shop-1", "a".repeat(48).as_str()] {
            assert!(validate_tenant(valid).is_ok(), "{valid}");
        }
        for invalid in [
            "a",
            "Shop",
            "-shop",
            "shop-",
            "sh op",
            "a".repeat(49).as_str(),
        ] {
            assert!(validate_tenant(invalid).is_err(), "{invalid}");
        }
        assert_eq!(uid().len(), 32);
        assert_eq!(hash("a").len(), 64);
    }
}
