//! A category write: its place in the tree and its data with translations per content language.
use crate::domain::graph_query;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use veyndo_core::document::view;
use veyndo_core::localized::{Languages, valid_locale_key};
use veyndo_core::{Result, bad};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Edit {
    pub revision: Option<i64>,
    pub parent_id: Option<String>,
    pub position: i32,
    pub data: Value,
}

/// A category as the merchant tree lists it.
#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: String,
    pub parent_id: Option<String>,
    pub position: i32,
    pub revision: i64,
    pub data: Value,
}

/// The fields of a category's `data` document, each kept as stored.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct CategoryData {
    pub active: Value,
    pub visible: Value,
    pub r#type: Value,
    pub url: Value,
    pub translations: Value,
}

/// The texts of one category translation, each kept as stored.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct CategoryText {
    pub name: Value,
    pub description: Value,
    pub slug: Value,
}

/// The JSON schema of a category write, as capabilities and agents see it.
pub fn input_schema(create: bool) -> Value {
    let mut fields = json!({
        "parentId":{"type":["string","null"]},
        "position":{"type":"integer"},
        "revision":{"type":"integer","minimum":1},
        "data":{"type":"object","required":["active","visible","type","translations"],"properties":{
            "active":{"type":"boolean"},"visible":{"type":"boolean"},
            "type":{"type":"string","enum":["page","folder","structuring","link"]},
            "translations":{"type":"object","description":"Content locale keys (for example en-GB), each containing name and optional description/slug. Include the shop main language."}
        }}
    });
    if create && let Some(fields) = fields.as_object_mut() {
        fields.insert("id".into(), json!({"type":"string"}));
    }
    json!({"type":"object","additionalProperties":false,"properties":fields,"required":if create {json!(["position","data"])} else {json!(["revision","position","data"])}})
}

impl Edit {
    pub fn parse(v: Value) -> Result<Self> {
        let edit: Edit = serde_json::from_value(v).map_err(|_| bad("Invalid category edit"))?;
        edit.validate()?;
        Ok(edit)
    }

    /// The shape of the data, independent of the shop: type, flags, bounded translations and HTTPS links.
    pub fn validate(&self) -> Result<()> {
        let d = &self.data;
        let data: CategoryData = view(d);
        if !d.is_object()
            || d.to_string().len() > 16000
            || !data.active.is_boolean()
            || !data.visible.is_boolean()
            || !["page", "folder", "structuring", "link"]
                .contains(&data.r#type.as_str().unwrap_or(""))
        {
            return Err(bad("Invalid category data"));
        }
        if d.get("displayNestedProducts")
            .is_some_and(|v| !v.is_boolean())
        {
            return Err(bad("Invalid nested category listing flag"));
        }
        let translations = data
            .translations
            .as_object()
            .filter(|m| !m.is_empty() && m.len() <= 100)
            .ok_or(bad("Category translations required"))?;
        for (lang, tr) in translations {
            let tr: CategoryText = view(tr);
            if !valid_locale_key(lang)
                || tr
                    .name
                    .as_str()
                    .is_some_and(|s| s.is_empty() || s.len() > 200)
                || tr.description.as_str().is_some_and(|s| s.len() > 4000)
                || tr
                    .slug
                    .as_str()
                    .is_some_and(|s| s.len() > 200 || s.contains(['?', '#', '/']))
            {
                return Err(bad("Invalid category translation"));
            }
        }
        if data.r#type == "link"
            && !data.url.as_str().is_some_and(|s| {
                reqwest::Url::parse(s).is_ok_and(|u| {
                    u.scheme() == "https" && u.username().is_empty() && u.password().is_none()
                })
            })
        {
            return Err(bad("Category links require HTTPS"));
        }
        Ok(())
    }

    /// The checks that depend on the shop: the saved fact query and names in enabled languages including the main one.
    pub fn validate_for(&self, languages: Languages) -> Result<()> {
        graph_query::validate(&self.data, languages)?;
        let data: CategoryData = view(&self.data);
        let names: Value = data
            .translations
            .as_object()
            .into_iter()
            .flatten()
            .map(|(key, tr)| (key.clone(), view::<CategoryText>(tr).name))
            .collect();
        languages.validate_names(&names, 200)
    }
}

#[cfg(test)]
mod tests {
    use super::Edit;
    use serde_json::json;
    use veyndo_core::localized::Languages;

    fn edit() -> Edit {
        let mut d = json!({"active":false,"visible":true,"type":"page","translations":{}});
        for l in ["en", "de", "fr", "es"] {
            d["translations"][l] = json!({"name":"Catalog"});
        }
        Edit {
            revision: None,
            parent_id: None,
            position: 0,
            data: d,
        }
    }

    #[test]
    fn validates_all_locales_and_links() {
        let mut e = edit();
        assert!(e.validate().is_ok());
        e.data["type"] = json!("link");
        e.data["url"] = json!("javascript:alert(1)");
        assert!(e.validate().is_err());
    }

    #[test]
    fn names_use_enabled_languages_and_include_the_main_one() {
        let enabled: Vec<String> = ["es-ES", "de-DE"].iter().map(|l| (*l).to_owned()).collect();
        let languages = Languages {
            main: "es-ES",
            enabled: &enabled,
        };
        let mut e = edit();
        assert!(e.validate_for(languages).is_err());
        e.data["translations"] = json!({"es":{"name":"Catálogo"},"de":{"name":null}});
        assert!(e.validate_for(languages).is_ok());
        e.data["translations"] = json!({"de":{"name":"Katalog"}});
        assert!(e.validate_for(languages).is_err());
    }
}
