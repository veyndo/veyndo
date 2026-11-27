//! Example: list a shop's products page by page, then call a typed MCP tool. Run with `VEYNDO_URL=... VEYNDO_SHOP=... cargo run -p veyndo-sdk --example catalog`.
use serde_json::json;
use veyndo_sdk::tools::CatalogSearchInput;
use veyndo_sdk::{Client, ClientOptions, Error, RequestOptions, collect_pages};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let client = Client::new(ClientOptions {
        base_url: std::env::var("VEYNDO_URL").unwrap_or_else(|_| "https://api.veyndo.ai".into()),
        tenant: Some(
            std::env::var("VEYNDO_SHOP")
                .unwrap_or_else(|_| "lumen-story-lab-15-o-98aa4751fa".into()),
        ),
        ..ClientOptions::default()
    });
    let catalog = client.catalog();
    let products = collect_pages(
        |after| {
            let body = json!({"limit": 20, "after": after});
            catalog.post_store_product(RequestOptions::json(body))
        },
        "elements",
        50,
    )
    .await?;
    for product in &products {
        println!("{}", product["id"]);
    }
    let search = CatalogSearchInput {
        query: Some("lamp".into()),
        limit: Some(3),
        ..CatalogSearchInput::default()
    };
    match client.mcp().call(&search).await {
        Ok(found) => println!("{found:#}"),
        Err(Error::Api(error)) => {
            eprintln!("{} {:?} {:?}", error.status, error.kind(), error.problems)
        }
        Err(other) => return Err(other),
    }
    Ok(())
}
