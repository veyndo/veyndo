//! Credential resolution and integration keys are owned by the identity component.
pub(crate) use veyndo_identity::http::integrations::{
    integration_create, integration_list, integration_revoke,
};
pub(crate) use veyndo_identity::store::credentials::{
    resolve_credential, resolve_identity_channel,
};
