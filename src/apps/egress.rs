//! Pinned egress clients live in the apps component; this module supplies the operator's private service origins.
use super::*;
pub(crate) async fn public_client(url: &reqwest::Url) -> Result<reqwest::Client> {
    veyndo_app_platform::service::egress::public_client(
        url,
        &crate::runtime_config::get().private_service_origins,
    )
    .await
}
