//! Contract of `veyndo_intelligence::ports::PublicEgress`, and a test adapter that refuses every destination.
use veyndo_core::{PortFuture, bad};
use veyndo_intelligence::ports::PublicEgress;

/// Destinations no implementation may reach: plain HTTP, loopback, private, link-local and metadata addresses.
pub const REFUSED: [&str; 7] = [
    "http://example.com/image.png",
    "https://127.0.0.1/image.png",
    "https://[::1]/image.png",
    "https://10.1.2.3/image.png",
    "https://192.168.0.10/image.png",
    "https://169.254.169.254/latest/meta-data",
    "https://localhost/image.png",
];

/// Every refused destination yields no client.
pub async fn refuses_private_destinations(egress: &dyn PublicEgress) {
    for url in REFUSED {
        let url = reqwest::Url::parse(url).expect("contract URL");
        assert!(
            egress.client(&url).await.is_err(),
            "PublicEgress: a client for {url} was issued"
        );
    }
}

/// The test adapter: no network at all.
pub struct Offline;

impl PublicEgress for Offline {
    fn client<'a>(&'a self, _url: &'a reqwest::Url) -> PortFuture<'a, reqwest::Client> {
        Box::pin(async { Err(bad("Offline test egress")) })
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn the_test_adapter_meets_the_contract() {
        super::refuses_private_destinations(&super::Offline).await;
    }
}
