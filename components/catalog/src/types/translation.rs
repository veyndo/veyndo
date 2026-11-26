//! The translated texts of a product in one language; `None` inherits along the language chain.
#[derive(Clone, sqlx::FromRow)]
pub struct ProductText {
    pub product_id: String,
    pub language_id: String,
    pub name: Option<String>,
    pub description: Option<String>,
}
