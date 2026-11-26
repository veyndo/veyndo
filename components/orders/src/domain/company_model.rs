//! Company profile admission, sparse channel inheritance and structured-address print projection.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::{Result, bad};
use veyndo_shipping::domain::geography;
use veyndo_shop::types::settings::Settings;
pub const FIELDS: &[&str] = &[
    "name",
    "address",
    "street",
    "houseNumber",
    "additionalAddressLine1",
    "additionalAddressLine2",
    "postalCode",
    "city",
    "country",
    "countryStateId",
    "taxId",
    "vatId",
    "economicId",
    "legalForm",
    "registrationNumber",
    "registerType",
    "registerCourt",
    "managingDirectors",
    "legalRepresentatives",
    "contentResponsible",
    "contentResponsibleAddress",
    "supervisoryAuthority",
    "professionalChamber",
    "professionalTitle",
    "professionalCountry",
    "professionalRulesUrl",
    "shareCapital",
    "outstandingCapital",
    "liquidationNotice",
    "email",
    "phoneNumber",
    "website",
    "bankName",
    "iban",
    "bic",
    "logoId",
    "brandName",
    "legalNotice",
    "responsibilityScope",
];
pub const LOCALIZED: &[&str] = &["brandName", "legalNotice", "responsibilityScope"];
/// The single-line company address derived from its parts.
#[derive(Serialize)]
struct Address {
    address: String,
}

/// The shop's default brand name on its public company data.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Brand {
    brand_name: String,
}

/// The public company logo and the storefront path that serves it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PublicLogo<'a> {
    logo_id: &'a str,
    logo_url: String,
}

pub fn resolve(base: &Value, overrides: &Value) -> Value {
    let mut value = base.clone();
    if !value.is_object() {
        value = json!({});
    }
    if let Some(patch) = overrides.as_object() {
        for (key, field) in patch {
            if field.is_null() {
                continue;
            }
            if LOCALIZED.contains(&key.as_str()) && field.is_object() {
                let mut text = value[key].as_object().cloned().unwrap_or_default();
                for (locale, text_value) in field.as_object().unwrap() {
                    if !text_value.is_null() {
                        text.insert(locale.clone(), text_value.clone());
                    }
                }
                value[key] = Value::Object(text);
            } else {
                value[key] = field.clone();
            }
        }
    }
    if view::<CompanyFields>(&value)
        .street
        .as_str()
        .is_some_and(|s| !s.trim().is_empty())
    {
        let address = address(&value);
        // The resolved value is an object, so the merge cannot fail.
        let _ = merge(&mut value, Address { address });
    }
    value
}
pub fn address(v: &Value) -> String {
    let get = |k: &str| v[k].as_str().unwrap_or("").trim();
    if get("street").is_empty() {
        return get("address").into();
    }
    [
        format!("{} {}", get("street"), get("houseNumber"))
            .trim()
            .to_string(),
        get("additionalAddressLine1").into(),
        get("additionalAddressLine2").into(),
        format!("{} {}", get("postalCode"), get("city"))
            .trim()
            .into(),
        get("countryStateId").into(),
        get("country").into(),
    ]
    .into_iter()
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join(", ")
}
/// The company fields a complete master data record must carry consistently.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct CompanyFacts {
    #[serde(deserialize_with = "lenient")]
    name: Option<String>,
    #[serde(deserialize_with = "lenient")]
    street: Option<String>,
    #[serde(deserialize_with = "lenient")]
    city: Option<String>,
    #[serde(deserialize_with = "lenient")]
    country: Option<String>,
    #[serde(deserialize_with = "lenient")]
    country_state_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    website: Option<String>,
    #[serde(deserialize_with = "lenient")]
    professional_rules_url: Option<String>,
}

pub fn validate(data: &Value, settings: &Settings, partial: bool) -> Result<()> {
    let object = data.as_object().ok_or(bad("Master data object required"))?;
    for (key, v) in object {
        if !FIELDS.contains(&key.as_str()) {
            return Err(bad("Unknown company field"));
        }
        if v.is_null() && partial {
            continue;
        }
        if LOCALIZED.contains(&key.as_str()) {
            let texts = v
                .as_object()
                .filter(|v| v.len() <= 100)
                .ok_or(bad("Localized company text required"))?;
            for (locale, text) in texts {
                if !settings.languages().allowed(locale)
                    || !text.is_null()
                        && text.as_str().is_none_or(|s| {
                            s.len() > if key == "brandName" { 500 } else { 8000 }
                                || s.contains('\0')
                        })
                {
                    return Err(bad("Invalid localized company text"));
                }
            }
        } else if v.as_str().is_none_or(|s| s.len() > 500 || s.contains('\0')) {
            return Err(bad("Invalid company field"));
        }
    }
    if partial {
        return Ok(());
    }
    let company: CompanyFacts = view(data);
    if company.name.as_deref().is_none_or(|s| s.trim().is_empty()) || address(data).is_empty() {
        return Err(bad("Company name and address required"));
    }
    if company.street.as_deref().is_some_and(|s| !s.is_empty())
        && [&company.city, &company.country]
            .iter()
            .any(|v| v.as_deref().is_none_or(|s| s.is_empty()))
    {
        return Err(bad("Structured address requires city and country"));
    }
    let country = company.country.as_deref().unwrap_or("");
    if !country.is_empty() {
        let row = geography::catalogue(&settings.country_definitions)
            .into_iter()
            .find(|c| c.code == country)
            .ok_or(bad("Unknown company country"))?;
        let state = company.country_state_id.as_deref().unwrap_or("");
        if !state.is_empty() && !row.states.iter().any(|s| s.code == state) {
            return Err(bad("Unknown company subdivision"));
        }
    } else if company
        .country_state_id
        .as_deref()
        .is_some_and(|s| !s.is_empty())
    {
        return Err(bad("Subdivision requires country"));
    }
    for url in [&company.website, &company.professional_rules_url] {
        if let Some(url) = url.as_deref().filter(|s| !s.is_empty()) {
            let parsed = reqwest::Url::parse(url).map_err(|_| bad("Invalid company URL"))?;
            if !["https", "http"].contains(&parsed.scheme())
                || parsed.host_str().is_none()
                || !parsed.username().is_empty()
                || parsed.password().is_some()
            {
                return Err(bad("Invalid company URL"));
            }
        }
    }
    Ok(())
}
/// The JSON fields `public` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PublicInput {
    #[serde(deserialize_with = "lenient")]
    logo_id: Option<String>,
}

/// What a storefront may show of the company: legal and brand details, never the bank account, the domestic tax ID or unlinked uploads.
pub fn public(
    data: &Value,
    text: &dyn Fn(&Value) -> String,
    default_brand: Option<String>,
    t: &str,
    channel: &str,
) -> Value {
    let input: PublicInput = view(data);
    let mut public = json!({});
    for key in [
        "name",
        "address",
        "street",
        "houseNumber",
        "additionalAddressLine1",
        "additionalAddressLine2",
        "postalCode",
        "city",
        "country",
        "countryStateId",
        "legalForm",
        "registrationNumber",
        "registerType",
        "registerCourt",
        "managingDirectors",
        "legalRepresentatives",
        "contentResponsible",
        "contentResponsibleAddress",
        "vatId",
        "economicId",
        "email",
        "phoneNumber",
        "website",
        "supervisoryAuthority",
        "professionalChamber",
        "professionalTitle",
        "professionalCountry",
        "professionalRulesUrl",
        "shareCapital",
        "outstandingCapital",
        "liquidationNotice",
    ] {
        if let Some(value) = data.get(key) {
            public[key] = value.clone();
        }
    }
    for key in LOCALIZED {
        public[*key] = json!(text(&data[*key]));
    }
    // The public projection is an object, so the merges cannot fail.
    if let Some(brand_name) = default_brand {
        let _ = merge(&mut public, Brand { brand_name });
    }
    if let Some(logo) = input.logo_id.as_deref().filter(|s| !s.is_empty()) {
        let _ = merge(
            &mut public,
            PublicLogo {
                logo_id: logo,
                logo_url: format!("/store-api/company-logo/{logo}?shop={t}&channel={channel}"),
            },
        );
    }
    public
}

/// The company fields the master data checks read.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct CompanyFields {
    pub logo_id: Value,
    pub name: Value,
    pub street: Value,
}

/// A stored company record with its revisions, as a save request sends it back or `records` returns it.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct CompanyRecord {
    pub revision: Value,
    pub base_revision: Value,
    pub data: Value,
    pub inherited: Value,
}

#[cfg(test)]
mod tests {
    /// The typed company patches write exactly what the indexed assignments wrote.
    #[test]
    fn typed_company_patches_match_the_indexed_writes() {
        use super::{Address, Brand, PublicLogo};
        use veyndo_core::document::merge;
        let mut indexed = serde_json::json!({"name":"Shop"});
        indexed["address"] = serde_json::json!("Street 1");
        indexed["brandName"] = serde_json::json!("Brand");
        indexed["logoId"] = serde_json::json!("l1");
        indexed["logoUrl"] = serde_json::json!("/store-api/company-logo/l1?shop=t&channel=c");
        let mut typed = serde_json::json!({"name":"Shop"});
        merge(
            &mut typed,
            Address {
                address: "Street 1".into(),
            },
        )
        .unwrap();
        merge(
            &mut typed,
            Brand {
                brand_name: "Brand".into(),
            },
        )
        .unwrap();
        merge(
            &mut typed,
            PublicLogo {
                logo_id: "l1",
                logo_url: "/store-api/company-logo/l1?shop=t&channel=c".into(),
            },
        )
        .unwrap();
        assert_eq!(typed, indexed);
    }

    use super::{address, public, resolve, validate};
    use serde_json::{Value, json};
    use veyndo_shop::types::settings::Settings;
    #[test]
    fn sparse_inheritance_preserves_empty_and_localized_null() {
        let b = json!({"name":"Base","phoneNumber":"111","brandName":{"es":"Marca","de":"Marke"},"street":"Road","houseNumber":"2","city":"Town","country":"DE"});
        let p =
            json!({"name":null,"phoneNumber":"","houseNumber":"9","brandName":{"de":null,"es":""}});
        let v = resolve(&b, &p);
        assert_eq!(v["name"], "Base");
        assert_eq!(v["phoneNumber"], "");
        assert_eq!(v["brandName"]["de"], "Marke");
        assert_eq!(v["brandName"]["es"], "");
        assert_eq!(v["address"], "Road 9, Town, DE");
    }
    #[test]
    fn legacy_address_is_not_guessed() {
        assert_eq!(
            address(&json!({"address":"Unparsed 4, Anywhere"})),
            "Unparsed 4, Anywhere"
        );
    }
    fn settings() -> Settings {
        serde_json::from_str(include_str!("../../../../fixtures/demo-settings.json")).unwrap()
    }

    fn company() -> Value {
        json!({"name":"Shop GmbH","street":"Weg","houseNumber":"1","postalCode":"10115","city":"Berlin","country":"DE",
            "website":"https://shop.example","brandName":{"de-DE":"Marke","en-GB":null}})
    }

    #[test]
    fn a_complete_profile_needs_name_address_known_places_and_plain_urls() {
        let s = settings();
        assert!(validate(&company(), &s, false).is_ok());
        assert!(
            validate(
                &json!({"name":"Shop","address":"Legacy 1, Town"}),
                &s,
                false
            )
            .is_ok()
        );
        for (key, value) in [
            ("name", json!(" ")),
            ("name", json!(7)),
            ("city", json!("")),
            ("country", json!("ZZ")),
            ("countryStateId", json!("DE-XX")),
            ("website", json!("ftp://shop.example")),
            ("website", json!("https://user:secret@shop.example")),
            ("professionalRulesUrl", json!("not a url")),
            ("brandName", json!("Marke")),
            ("brandName", json!({"xx-XX":"Marke"})),
            ("brandName", json!({"de-DE":"x".repeat(501)})),
            ("legalNotice", json!({"de-DE":"a\0b"})),
            ("password", json!("x")),
            ("email", json!("x".repeat(501))),
        ] {
            let mut changed = company();
            changed[key] = value;
            assert!(validate(&changed, &s, false).is_err(), "{key}");
        }
        assert!(validate(&json!([]), &s, false).is_err());
        assert!(validate(&json!({}), &s, false).is_err());
        assert!(
            validate(
                &json!({"name":"Shop","address":"Legacy 1","countryStateId":"DE-BE"}),
                &s,
                false
            )
            .is_err()
        );
    }

    #[test]
    fn a_channel_override_may_be_sparse_but_not_unknown() {
        let s = settings();
        assert!(validate(&json!({}), &s, true).is_ok());
        assert!(validate(&json!({"name":null,"phoneNumber":""}), &s, true).is_ok());
        assert!(validate(&json!({"password":null}), &s, true).is_err());
        assert!(validate(&json!({"name":7}), &s, true).is_err());
    }

    #[test]
    fn the_public_profile_leaves_out_bank_tax_and_unknown_fields() {
        let mut data = company();
        data["iban"] = json!("DE89370400440532013000");
        data["bankName"] = json!("Example Bank");
        data["taxId"] = json!("12/345/67890");
        data["vatId"] = json!("DE123456789");
        data["logoId"] = json!("logo-1");
        let text = |v: &Value| v["de-DE"].as_str().unwrap_or("").to_owned();
        let v = public(&data, &text, None, "shop", "default");
        assert_eq!(v["name"], "Shop GmbH");
        assert_eq!(v["vatId"], "DE123456789");
        assert_eq!(v["brandName"], "Marke");
        assert_eq!(v["legalNotice"], "");
        assert_eq!(
            v["logoUrl"],
            "/store-api/company-logo/logo-1?shop=shop&channel=default"
        );
        for hidden in ["iban", "bankName", "bic", "taxId"] {
            assert!(v.get(hidden).is_none(), "{hidden}");
        }
        let branded = public(
            &json!({"logoId":""}),
            &text,
            Some("Default brand".into()),
            "shop",
            "default",
        );
        assert_eq!(branded["brandName"], "Default brand");
        assert!(branded.get("logoUrl").is_none());
    }
}
