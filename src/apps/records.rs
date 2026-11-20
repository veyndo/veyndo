//! The app record store lives in the apps component; this module keeps the short path for the app adapters.
pub(crate) use veyndo_app_platform::store::records::{
    get, install, key, lock_key, project, reset_preview, rows,
};
