//! A rejected pricing input becomes a client error of the shared error contract.
impl From<crate::ValidationError> for veyndo_core::Error {
    fn from(error: crate::ValidationError) -> Self {
        veyndo_core::bad(error.to_string())
    }
}
