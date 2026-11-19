//! Which media a product may reference: images and at most four uploaded 3D models, with safe URLs and bounded labels.
use serde_json::Value;
use veyndo_core::{Result, bad};

/// Media types and the 3D model rules, independent of what is stored.
pub fn validate_shape(media: &[Value]) -> Result<()> {
    if media.iter().filter(|m| m["type"] == "model").count() > 4 {
        return Err(bad("Maximum four 3D models per product"));
    }
    for item in media {
        if item
            .get("type")
            .is_some_and(|v| v != "image" && v != "model")
        {
            return Err(bad("Unknown product media type"));
        }
        if item["type"] == "model" {
            if !item["url"]
                .as_str()
                .is_some_and(|u| u.starts_with("/store-api/assets/"))
            {
                return Err(bad("3D models require an uploaded GLB asset"));
            }
            if item.get("modelVariant").is_some_and(|v| {
                !v.is_null()
                    && v.as_str()
                        .is_none_or(|s| s.len() > 100 || s.chars().any(char::is_control))
            }) {
                return Err(bad("Invalid 3D material variant"));
            }
        } else if item.get("modelVariant").is_some() {
            return Err(bad("Material variants require a 3D model"));
        }
    }
    Ok(())
}

/// The whole media list of a product: bounded, with local or credential-free HTTPS URLs and bounded labels.
pub fn validate(media: &Value) -> Result<()> {
    let media = media
        .as_array()
        .filter(|a| a.len() <= 20)
        .ok_or(bad("Product media must be a bounded array"))?;
    for m in media {
        let url = m["url"].as_str().ok_or(bad("Media URL required"))?;
        let local = url.starts_with("/store-api/assets/")
            || url.starts_with("/assets/")
            || url.starts_with("/media/");
        if !local
            && !reqwest::Url::parse(url).is_ok_and(|u| {
                u.scheme() == "https" && u.username().is_empty() && u.password().is_none()
            })
        {
            return Err(bad("Unsafe product media URL"));
        }
        if m["id"].as_str().is_none_or(|s| s.len() > 100)
            || m["view"].as_str().is_none_or(|s| s.len() > 100)
        {
            return Err(bad("Invalid product media labels"));
        }
    }
    validate_shape(media)
}

/// The asset ID a model item refers to; refuses fragments, foreign shops and nested paths.
pub fn model_asset(url: &str, tenant: &str) -> Result<String> {
    let parsed = reqwest::Url::parse(&format!("https://local{url}"))
        .map_err(|_| bad("Invalid model asset URL"))?;
    if parsed.fragment().is_some()
        || parsed
            .query_pairs()
            .any(|(k, v)| k != "shop" || v != tenant)
    {
        return Err(bad("Model asset belongs to another shop"));
    }
    parsed
        .path()
        .strip_prefix("/store-api/assets/")
        .filter(|s| !s.is_empty() && !s.contains('/'))
        .map(str::to_owned)
        .ok_or(bad("Invalid model asset URL"))
}

#[cfg(test)]
mod tests {
    use super::{model_asset, validate};
    use serde_json::json;

    #[test]
    fn media_urls_are_local_or_credential_free_https() {
        let item = |url: &str| json!([{"url":url,"id":"a","view":"front"}]);
        for url in ["/media/a.jpg", "/assets/a.jpg", "https://cdn.example/a.jpg"] {
            assert!(validate(&item(url)).is_ok(), "{url}");
        }
        for url in [
            "http://cdn.example/a.jpg",
            "https://user:pw@cdn.example/a.jpg",
            "javascript:alert(1)",
        ] {
            assert!(validate(&item(url)).is_err(), "{url}");
        }
        assert!(validate(&json!({"url":"/media/a"})).is_err());
        assert!(validate(&json!([{"url":"/media/a","id":"a"}])).is_err());
    }

    #[test]
    fn a_model_asset_belongs_to_the_same_shop() {
        assert_eq!(
            model_asset("/store-api/assets/m1?shop=a", "a").unwrap(),
            "m1"
        );
        assert_eq!(model_asset("/store-api/assets/m1", "a").unwrap(), "m1");
        assert!(model_asset("/store-api/assets/m1?shop=b", "a").is_err());
        assert!(model_asset("/store-api/assets/m1?shop=a#x", "a").is_err());
        assert!(model_asset("/store-api/assets/m1/x?shop=a", "a").is_err());
        assert!(model_asset("/assets/m1", "a").is_err());
    }
}
