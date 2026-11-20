//! Shared address wire contract and original bounded syntax normalization; hosts retain geography admission.
use super::{ValidationError, country_default};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Address {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub first_name: String,
    #[serde(default)]
    pub last_name: String,
    #[serde(default)]
    pub salutation_id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub company: String,
    #[serde(default)]
    pub department: String,
    #[serde(default)]
    pub vat_id: String,
    pub street: String,
    #[serde(alias = "zipcode")]
    pub postal_code: String,
    pub city: String,
    #[serde(default = "country_default")]
    pub country: String,
    #[serde(default)]
    pub country_state_id: String,
    #[serde(default)]
    pub additional_address_line1: String,
    #[serde(default)]
    pub additional_address_line2: String,
    #[serde(default)]
    pub phone_number: String,
}
impl Address {
    pub fn validate(&mut self) -> Result<(), ValidationError> {
        if self.name.trim().is_empty() {
            self.name = format!("{} {}", self.first_name.trim(), self.last_name.trim())
                .trim()
                .into();
        }
        for s in [&self.name, &self.street, &self.postal_code, &self.city] {
            if s.trim().is_empty() || s.len() > 160 {
                return Err(ValidationError::new(
                    "Complete address fields required (maximum 160 characters)",
                ));
            }
        }
        if self.country.len() != 2 || !self.country.bytes().all(|c| c.is_ascii_uppercase()) {
            return Err(ValidationError::new(
                "Address country must be an ISO country code",
            ));
        }
        for s in [
            &self.first_name,
            &self.last_name,
            &self.salutation_id,
            &self.title,
            &self.company,
            &self.department,
            &self.vat_id,
            &self.country_state_id,
            &self.additional_address_line1,
            &self.additional_address_line2,
            &self.phone_number,
        ] {
            if s.len() > 160 || s.chars().any(|c| c.is_control()) {
                return Err(ValidationError::new("Invalid address field"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Address;
    use serde_json::{Value, json};

    fn address(extra: Value) -> Address {
        let mut v = json!({"firstName":" Ada ","lastName":"Lovelace","street":"Weg 1","postalCode":"10115","city":"Berlin"});
        for (key, value) in extra.as_object().unwrap() {
            v[key] = value.clone();
        }
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn a_missing_name_is_built_from_its_parts_and_the_country_defaults() {
        let mut a = address(json!({}));
        assert!(a.validate().is_ok());
        assert_eq!(a.name, "Ada Lovelace");
        assert_eq!(a.country, "DE");
        let mut named: Address = serde_json::from_value(
            json!({"name":"A. Lovelace","street":"Weg 1","zipcode":"80331","city":"Munich"}),
        )
        .unwrap();
        assert!(named.validate().is_ok());
        assert_eq!(
            (named.name.as_str(), named.postal_code.as_str()),
            ("A. Lovelace", "80331")
        );
    }

    #[test]
    fn incomplete_oversized_or_control_text_is_refused() {
        for extra in [
            json!({"firstName":"","lastName":""}),
            json!({"street":" "}),
            json!({"city":"x".repeat(161)}),
            json!({"country":"de"}),
            json!({"country":"DEU"}),
            json!({"phoneNumber":"1\n2"}),
            json!({"company":"x".repeat(161)}),
        ] {
            assert!(address(extra.clone()).validate().is_err(), "{extra}");
        }
    }
}
