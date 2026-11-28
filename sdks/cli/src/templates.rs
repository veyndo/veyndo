//! App starter templates: the same app-owned pattern Manifests App Studio clones (components/apps/ui/admin/developer/app-recipes.ts).
use serde_json::Value;

/// One starter template.
pub struct Template {
    /// Template name on the command line (the Studio recipe ID).
    pub name: &'static str,
    /// What the template builds.
    pub summary: &'static str,
    /// The tutorial that explains it.
    pub tutorial: &'static str,
    /// The pattern Manifest.
    pub manifest: &'static str,
}

/// Every template, in the order App Studio offers the patterns.
pub const TEMPLATES: &[Template] = &[
    Template {
        name: "advisor",
        summary: "Product care advisor: product editing tab and shopper advice cards",
        tutorial: "docs/app-tutorials/product-advisor.md",
        manifest: include_str!("../../../extensions/apps/product-advisor/manifest.json"),
    },
    Template {
        name: "returns",
        summary: "Returns workspace: private order cases, status and internal notes",
        tutorial: "docs/app-tutorials/returns-workspace.md",
        manifest: include_str!("../../../extensions/apps/returns-workspace/manifest.json"),
    },
    Template {
        name: "erp",
        summary: "ERP order handoff: mapping UI, order events and an external service action",
        tutorial: "docs/app-tutorials/erp-handoff.md",
        manifest: include_str!("../../../extensions/apps/erp-handoff/manifest.json"),
    },
    Template {
        name: "edi",
        summary: "EDI workspace: B2B documents through an external service",
        tutorial: "extensions/apps/edi/README.md",
        manifest: include_str!("../../../extensions/apps/edi/manifest.json"),
    },
    Template {
        name: "furniture",
        summary: "Furniture configurator: storefront options with a typed Wasm surcharge",
        tutorial: "extensions/apps/furniture-configurator/README.md",
        manifest: include_str!("../../../extensions/apps/furniture-configurator/manifest.json"),
    },
];

/// The template of this name.
pub fn find(name: &str) -> Option<&'static Template> {
    TEMPLATES.iter().find(|t| t.name == name)
}

/// A Manifest ID: lowercase letters, digits and underscores, starting with a letter.
pub fn valid_id(id: &str) -> bool {
    let mut chars = id.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && (2..=64).contains(&id.len())
}

/// The template Manifest with its ID replaced and its version reset, as Studio's `appRecipe` does.
pub fn manifest(template: &Template, id: &str) -> Value {
    let mut manifest: Value = serde_json::from_str(template.manifest).unwrap_or_default();
    manifest["id"] = Value::String(id.to_string());
    manifest["version"] = Value::String("0.1.0".into());
    manifest
}

/// The README of a new app project.
pub fn readme(template: &Template, id: &str) -> String {
    format!(
        "# {id}\n\nCreated with `veyndo app new {name}` from the {name} pattern: {summary}.\n\
Tutorial: {tutorial} in the Veyndo repository.\n\n\
```bash\nveyndo app validate\n```\n\n\
```bash\nveyndo app deploy\n```\n\n\
`validate` reviews the package on the server without writing anything. `deploy` installs it\n\
into a private sandbox of your shop; review it there and publish with `veyndo sandbox release`.\n\
Add a `contract.json` beside the Manifest to declare agent tool facts, ports and rollbacks\n\
(see docs/app-platform.md, \"Signed packages, versions and rollback\"). Every published\n\
version is immutable: raise `version` before deploying a changed package.\n",
        name = template.name,
        summary = template.summary,
        tutorial = template.tutorial,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_template_is_a_manifest_with_an_id_and_english_name() {
        for template in TEMPLATES {
            let manifest: Value = serde_json::from_str(template.manifest).unwrap();
            assert!(
                valid_id(manifest["id"].as_str().unwrap()),
                "{}",
                template.name
            );
            assert!(manifest["name"]["en"].is_string(), "{}", template.name);
            let renamed = super::manifest(template, "my_app");
            assert_eq!(renamed["id"], "my_app");
            assert_eq!(renamed["version"], "0.1.0");
        }
    }

    #[test]
    fn ids_are_snake_case() {
        assert!(valid_id("my_app2"));
        assert!(!valid_id("My-App"));
        assert!(!valid_id("2app"));
        assert!(!valid_id("a"));
    }
}
