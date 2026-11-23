//! Model navigation lives in the intelligence component; the native working view supplies its entity.
pub(crate) use veyndo_intelligence::service::navigation::{
    Navigation, admit, schema, validate_view,
};
impl veyndo_intelligence::service::navigation::Entity for super::view::View {
    fn entity(&self) -> &str {
        &self.entity
    }
}
