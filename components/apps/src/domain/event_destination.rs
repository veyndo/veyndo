//! Where a package may have its events delivered: public HTTPS only.
use veyndo_core::{Result, bad};

/// Admits a public HTTPS destination without credentials, query or fragment; private addresses are refused.
pub fn destination(url: &str) -> Result<reqwest::Url> {
    let u = reqwest::Url::parse(url).map_err(|_| bad("Invalid event destination URL"))?;
    if u.scheme() != "https"
        || !u.username().is_empty()
        || u.password().is_some()
        || u.fragment().is_some()
        || u.query().is_some()
        || url.len() > 2048
    {
        return Err(bad(
            "Event destination requires public HTTPS without credentials, query or fragment",
        ));
    }
    if let Ok(ip) = u.host_str().unwrap_or("").parse()
        && !veyndo_runtime::network_policy::public_address(ip)
    {
        return Err(bad("Private event destinations are prohibited"));
    }
    Ok(u)
}

#[cfg(test)]
mod tests {
    use super::destination;

    #[test]
    fn destinations_are_public_https_without_credentials_query_or_fragment() {
        assert!(destination("https://hooks.example/events").is_ok());
        let long = format!("https://hooks.example/{}", "x".repeat(2048));
        for url in [
            "http://hooks.example/events",
            "https://user:secret@hooks.example/events",
            "https://hooks.example/events?token=1",
            "https://hooks.example/events#x",
            "https://127.0.0.1/events",
            "https://10.0.0.1/events",
            "not a url",
            long.as_str(),
        ] {
            assert!(destination(url).is_err(), "{url}");
        }
    }
}
