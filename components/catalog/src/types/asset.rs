//! A stored product file as merchant lists show it; the content itself never enters a list.
use serde::Serialize;
use serde_json::Value;

#[derive(Serialize, sqlx::FromRow)]
pub struct Asset {
    pub id: String,
    pub title: Value,
    pub filename: String,
    pub mime: String,
    pub kind: String,
    pub public: bool,
    pub digest: String,
    pub bytes: i32,
}

/// One uploaded file with its form fields; `grant` and `product` only exist on app upload surfaces.
pub struct Upload {
    pub bytes: Vec<u8>,
    pub filename: String,
    pub mime: String,
    pub kind: String,
    pub title: Value,
    pub grant: Option<String>,
    pub product: Option<String>,
}

/// A published attachment as the storefront lists it.
#[derive(sqlx::FromRow)]
pub struct Attachment {
    pub id: String,
    pub title: Value,
    pub filename: String,
    pub mime: String,
    pub bytes: i32,
}

/// A stored file with its content, ready to be delivered.
#[derive(sqlx::FromRow)]
pub struct StoredFile {
    /// The product the file belongs to; empty when the read did not need it.
    #[sqlx(default)]
    pub product_id: String,
    pub mime: String,
    pub filename: String,
    pub content: Vec<u8>,
}
