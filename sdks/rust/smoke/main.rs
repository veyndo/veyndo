//! Smoke test of the generated Rust SDK against a running Veyndo server (BASE_URL); synthetic accounts only. Run by scripts/sdk_smoke.py.
use serde_json::{Value, json};
use veyndo_sdk::tools::{CatalogSearchInput, MerchantProductsInput};
use veyndo_sdk::{Auth, Client, ClientOptions, Error, ProblemKind, RequestOptions, collect_pages};

fn expect_api(
    result: Result<Value, Error>,
    status: u16,
    kind: &ProblemKind,
) -> veyndo_sdk::ApiError {
    match result {
        Err(Error::Api(error)) => {
            assert_eq!(
                (error.status, error.kind()),
                (status, Some(kind)),
                "{error:?}"
            );
            assert!(!error.problems[0].detail.is_empty());
            error
        }
        other => panic!("expected HTTP {status} {kind:?}, got {other:?}"),
    }
}

async fn store_checks(store: &Client) {
    let complete = store
        .catalog()
        .post_store_product(RequestOptions::json(json!({"limit": 100})))
        .await
        .unwrap();
    let paged = collect_pages(
        |after| async move {
            let mut body = json!({"limit": 2});
            if let Some(after) = after {
                body["after"] = json!(after);
            }
            store
                .catalog()
                .post_store_product(RequestOptions::json(body))
                .await
        },
        "elements",
        100,
    )
    .await
    .unwrap();
    let complete = complete["elements"].as_array().unwrap();
    assert!(paged.len() > 2);
    assert_eq!(
        paged.iter().map(|p| &p["id"]).collect::<Vec<_>>(),
        complete.iter().map(|p| &p["id"]).collect::<Vec<_>>()
    );
    let rejected = store
        .catalog()
        .post_store_product(RequestOptions::json(json!({"limit": 0})))
        .await;
    expect_api(rejected, 400, &ProblemKind::ValidationError);
    let input = CatalogSearchInput {
        limit: Some(2),
        ..CatalogSearchInput::default()
    };
    let found = store.mcp().call(&input).await.unwrap();
    assert_eq!(found["elements"].as_array().unwrap().len(), 2);
    let invalid = CatalogSearchInput {
        limit: Some(0),
        ..CatalogSearchInput::default()
    };
    let error = expect_api(
        store.mcp().call(&invalid).await,
        400,
        &ProblemKind::ValidationError,
    );
    assert_eq!(error.problems[0].pointer.as_deref(), Some("/limit"));
    let denied = store.mcp().call(&MerchantProductsInput::default()).await;
    expect_api(denied, 403, &ProblemKind::PermissionDenied);
    match store.mcp().call_raw("no.such.tool", json!({})).await {
        Err(Error::Mcp { code, .. }) => assert_eq!(code, -32602),
        other => panic!("an unknown tool must be a protocol error: {other:?}"),
    }
}

async fn credential_checks(anonymous: &Client, suffix: &str) {
    let email = format!("sdk-rust-{suffix}@example.test");
    let account = anonymous
        .identity()
        .post_auth_register(RequestOptions::json(json!({
            "email": email, "name": "SDK smoke", "password": "Synthetic-sdk-2026!",
            "workspaceId": format!("sdk-rust-{suffix}")
        })))
        .await
        .unwrap();
    let workspace = account["workspace"].as_str().unwrap().to_string();
    let personal = anonymous.with(|o| {
        o.auth = Some(Auth::PersonalSession(
            account["token"].as_str().unwrap().into(),
        ));
        o.tenant = Some(workspace.clone());
    });
    let session = personal
        .identity()
        .get_auth_session(RequestOptions::default())
        .await
        .unwrap();
    assert_eq!(session["user"]["email"], json!(email));
    let key = personal
        .identity()
        .post_workspace_integrations(RequestOptions::json(
            json!({"name": "sdk-smoke", "permissions": ["catalog.read"], "expiresInDays": 1}),
        ))
        .await
        .unwrap();
    let integration = anonymous.with(|o| {
        o.auth = Some(Auth::IntegrationKey(key["key"].as_str().unwrap().into()));
        o.tenant = Some(workspace.clone());
    });
    let tools = integration.mcp().list_tools().await.unwrap();
    assert!(tools.iter().any(|tool| tool["name"] == "merchant.products"));
    let input = MerchantProductsInput {
        limit: Some(5),
        ..MerchantProductsInput::default()
    };
    assert!(integration.mcp().call(&input).await.unwrap()["elements"].is_array());
    let denied = integration
        .identity()
        .get_workspace_integrations(RequestOptions::default())
        .await;
    expect_api(denied, 403, &ProblemKind::PermissionDenied);
}

#[tokio::main]
async fn main() {
    let base_url = std::env::var("BASE_URL").expect("BASE_URL is required");
    let shop = std::env::var("SDK_SMOKE_SHOP").unwrap_or_else(|_| "atelier".into());
    let anonymous = Client::new(ClientOptions {
        base_url,
        ..ClientOptions::default()
    });
    let health = anonymous
        .host()
        .get_health(RequestOptions::default())
        .await
        .unwrap();
    assert_eq!(health["status"], "ok");
    let session = anonymous
        .identity()
        .get_auth_session(RequestOptions::default())
        .await;
    expect_api(session, 401, &ProblemKind::AuthenticationRequired);
    store_checks(&anonymous.with(|o| o.tenant = Some(shop))).await;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    credential_checks(&anonymous, &format!("{nanos:x}")).await;
    println!(
        "PASS Rust SDK: health, store pagination, typed HTTP/MCP failures, personal session, integration key and MCP tools"
    );
}
