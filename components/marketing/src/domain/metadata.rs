//! Revision-bound source facts a rule may read on products, customers and orders; secrets and pricing authority are excluded.
use serde_json::Value;
use veyndo_core::{Result, bad};

pub fn validate(entity: &str, data: &Value) -> Result<()> {
    if data.is_null() {
        return Ok(());
    }
    let object = data
        .as_object()
        .filter(|o| o.len() <= 40)
        .ok_or(bad("Rule metadata object required"))?;
    if data.to_string().len() > 16000 {
        return Err(bad("Rule metadata exceeds limit"));
    }
    let keys = match entity {
        "products" => &[
            "height",
            "width",
            "length",
            "weight",
            "volume",
            "closeout",
            "markAsTopseller",
            "isNew",
            "tags",
            "streamIds",
            "manufacturerId",
            "customFields",
            "categoryIds",
            "taxId",
            "propertyIds",
            "optionIds",
            "purchasePrices",
            "releaseDate",
            "createdAt",
        ][..],
        "customers" => &[
            "tags",
            "customFields",
            "affiliateCode",
            "campaignCode",
            "requestedGroupId",
            "newsletter",
            "createdByAdmin",
        ][..],
        "orders" => &[
            "tags",
            "customFields",
            "affiliateCode",
            "campaignCode",
            "createdByAdmin",
        ][..],
        _ => return Err(bad("Unknown rule entity")),
    };
    for (key, value) in object {
        if !keys.contains(&key.as_str()) {
            return Err(bad(format!("Unsupported rule metadata field: {key}")));
        }
        if ["height", "width", "length", "weight", "volume"].contains(&key.as_str())
            && value.as_f64().is_none_or(|n| !n.is_finite() || n < 0.)
        {
            return Err(bad("Dimension must be nonnegative"));
        }
        if [
            "closeout",
            "markAsTopseller",
            "isNew",
            "newsletter",
            "createdByAdmin",
        ]
        .contains(&key.as_str())
            && !value.is_boolean()
        {
            return Err(bad("Boolean rule metadata required"));
        }
        if [
            "tags",
            "streamIds",
            "categoryIds",
            "propertyIds",
            "optionIds",
        ]
        .contains(&key.as_str())
            && value.as_array().is_none_or(|a| {
                a.len() > 100
                    || a.iter()
                        .any(|s| s.as_str().is_none_or(|s| s.is_empty() || s.len() > 100))
            })
        {
            return Err(bad("Bounded identifier list required"));
        }
        if key == "customFields" && !value.is_object() {
            return Err(bad("Custom field object required"));
        }
        if key == "purchasePrices"
            && !value.is_null()
            && (!value.is_object()
                || value.as_object().unwrap().iter().any(|(k, v)| {
                    !["net", "gross"].contains(&k.as_str())
                        || v.as_f64().is_none_or(|n| !n.is_finite() || n < 0.)
                }))
        {
            return Err(bad("Nonnegative net/gross purchase prices required"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate;
    use serde_json::json;
    #[test]
    fn metadata_cannot_carry_pricing_authority_or_negative_dimensions() {
        assert!(validate("products", &json!({"price":0})).is_err());
        assert!(validate("products", &json!({"width":-1})).is_err());
    }
}
