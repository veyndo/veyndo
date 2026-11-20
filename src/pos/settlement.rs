//! Cash admission and the POS order annotation are owned by the POS component.
pub(crate) use veyndo_pos::service::settlement::{
    check_locked, payment_captured, settle, validate_settlement,
};
