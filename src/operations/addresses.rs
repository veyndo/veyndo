//! Merchant address operations live in the customers component.
pub(super) use veyndo_customers::http::merchant_crm::{
    addresses as list, create_address as create, remove_address as remove, save_address as save,
};
