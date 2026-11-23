//! Entries of the address book are owned by the customers component.
pub(crate) use veyndo_customers::service::address_entries::{
    address_delete, address_get, address_save, address_save_conn,
};
