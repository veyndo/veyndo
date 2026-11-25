//! Public navigation: a hidden category hides its whole branch, and every entry is shown in the visitor's language.
use crate::domain::content::translated_object;
use crate::types::category::{Category, CategoryData, CategoryText};
use serde_json::{Value, json};
use std::collections::HashMap;
use veyndo_core::document::view;

/// The entries of the navigation in the given order, without hidden categories and their descendants.
pub fn entries(tree: &[Category], locale: &str, main: &str) -> Vec<Value> {
    // A hidden ancestor hides its entire navigation branch; direct active listings remain available.
    let parents: HashMap<&str, (Option<&str>, bool)> = tree
        .iter()
        .map(|c| {
            (
                c.id.as_str(),
                (
                    c.parent_id.as_deref(),
                    view::<CategoryData>(&c.data).visible != false,
                ),
            )
        })
        .collect();
    let visible = |id: &str| {
        let mut next = Some(id);
        while let Some(current) = next {
            match parents.get(current) {
                Some((parent, true)) => next = *parent,
                Some((_, false)) => return false,
                None => break,
            }
        }
        true
    };
    tree.iter()
        .filter(|c| visible(&c.id))
        .map(|c| {
            let data: CategoryData = view(&c.data);
            let tr: CategoryText = view(&translated_object(&data.translations, locale, main));
            json!({"id":c.id,"parentId":c.parent_id,"name":tr.name,"description":tr.description,"slug":tr.slug,"type":data.r#type,"url":data.url,"position":c.position})
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::entries;
    use crate::types::category::Category;
    use serde_json::{Value, json};

    fn category(id: &str, parent: Option<&str>, data: Value) -> Category {
        Category {
            id: id.into(),
            parent_id: parent.map(str::to_owned),
            position: 0,
            revision: 1,
            data,
        }
    }

    #[test]
    fn a_hidden_category_hides_its_branch_and_names_inherit_the_main_language() {
        let tree = [
            category(
                "root",
                None,
                json!({"type":"page","translations":{"es":{"name":"Catálogo"}}}),
            ),
            category(
                "lamps",
                Some("root"),
                json!({"type":"page","translations":{"es":{"name":"Lámparas","slug":"lamparas"},"de":{"name":"Leuchten"}}}),
            ),
            category(
                "hidden",
                Some("root"),
                json!({"visible":false,"type":"page","translations":{"es":{"name":"Oculto"}}}),
            ),
            category(
                "below",
                Some("hidden"),
                json!({"type":"page","translations":{"es":{"name":"Debajo"}}}),
            ),
        ];
        let list = entries(&tree, "de-DE", "es-ES");
        let ids: Vec<&str> = list.iter().map(|e| e["id"].as_str().unwrap()).collect();
        assert_eq!(ids, ["root", "lamps"]);
        assert_eq!(list[0]["name"], "Catálogo");
        assert_eq!(list[1]["name"], "Leuchten");
        assert_eq!(list[1]["slug"], "lamparas");
        assert_eq!(list[1]["parentId"], "root");
    }
}
