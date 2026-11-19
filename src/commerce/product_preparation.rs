//! Opt-in bounded catalogue preparation projection. Existing products, translations and image jobs remain the owners.
use super::*;
use veyndo_catalog::domain::preparation::{Source, filled_locales};
use veyndo_catalog::store::{media_jobs, translations};

pub(super) async fn attach(
    a: &App,
    tenant: &str,
    products: &mut [Value],
    sources: &[(String, String, String)],
) -> Result<()> {
    if products.is_empty() {
        return Ok(());
    }
    let ids = products
        .iter()
        .filter_map(|p| p["id"].as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    // Fetch only the already-authorized page, never the whole shop or binary media.
    let texts = translations::all_for_products(&a.core, tenant, &ids).await?;
    let jobs = media_jobs::latest(&a.core, tenant, &ids).await?;
    let languages = performance::languages(a).await?;
    let (settings, _) = config(a, tenant).await?;
    let language_of = |locale: &str| {
        languages
            .iter()
            .find(|l| l.locale == locale)
            .map(|l| l.id.clone())
    };
    for product in products {
        let id = product["id"].as_str().unwrap_or_default().to_owned();
        let source = sources
            .iter()
            .find(|(product, _, _)| *product == id)
            .map(|(_, name, description)| Source { name, description });
        let locales = |description: bool| {
            filled_locales(
                &id,
                source,
                &texts,
                language_of,
                settings.languages(),
                description,
            )
        };
        let job = jobs.iter().find(|job| job.product_id == id);
        product["preparation"] = json!({
            "namedLocales":locales(false),"describedLocales":locales(true),
            "locales":settings.locales,"mainLocale":settings.main_locale,"imageJob":job
        });
    }
    Ok(())
}
