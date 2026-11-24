//! Adapter providers of a shop: merchant-approved account onboarding, the public provider listing and the ledger status overview; only authenticated provider responses establish account bindings.
use crate::domain::attempt::Attempt;
use crate::domain::service_config::approval_url;
use crate::domain::settlements::summary;
use crate::ports::{Gateway, Host};
use crate::store::attempts::attempt;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::{
    Error, RequestContext, Result, State, StatusCode, bad, conflict, process_config,
};
use veyndo_identity::{merchant, permit};

/// Starts, inspects or disconnects the provider account of one sales channel; every effect needs `approve`.
pub async fn onboarding(
    (host, gateway): (&dyn Host, &dyn Gateway),
    a: &State,
    h: &RequestContext,
    id: &str,
    input: &Value,
) -> Result<Value> {
    let t = merchant(h)?;
    permit(h, "payments.manage")?;
    let m = gateway.package(&t, id).await?;
    let contract = m
        .payment_provider
        .as_ref()
        .ok_or(bad("App has no payment provider contract"))?;
    let op = input["operation"].as_str().unwrap_or("status");
    if !["status", "start", "disconnect"].contains(&op)
        || (op != "status" && input["approve"] != true)
    {
        return Err(bad("Supported onboarding operation and approval required"));
    }
    let channel = input["channel"].as_str().unwrap_or("default");
    let owned: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sales_channels WHERE tenant=$1 AND id=$2)")
            .bind(&t)
            .bind(channel)
            .fetch_one(&a.db)
            .await?;
    if !owned {
        return Err(Error(
            StatusCode::NOT_FOUND,
            "Sales channel not found".into(),
        ));
    }
    let configured = &process_config::get().payment_services[id][&m.version];
    let environment = input["environment"]
        .as_str()
        .or_else(|| configured["defaultEnvironment"].as_str())
        .or_else(|| configured["environment"].as_str())
        .ok_or(Error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Payment service not configured".into(),
        ))?;
    let key = input["requestKey"]
        .as_str()
        .filter(|v| v.len() >= 8 && v.len() <= 128)
        .ok_or(bad("Onboarding requestKey required"))?;
    let body = json!({"apiVersion":"1","provider":id,"adapterVersion":m.version,"tenant":t,"channel":channel,"environment":environment,"operation":op,"requestKey":key,"country":input["country"]});
    let result = gateway
        .call(&t, id, &m.version, environment, "onboarding", &body)
        .await?;
    if result["apiVersion"] != "1"
        || result["adapterVersion"] != m.version
        || result["tenant"] != t
        || result["provider"] != id
        || result["environment"] != environment
        || result["channel"] != channel
    {
        return Err(bad("Onboarding account identity mismatch"));
    }
    let account = result["accountRef"]
        .as_str()
        .filter(|v| !v.is_empty() && v.len() <= 200)
        .ok_or(bad("Provider account reference missing"))?;
    let ready = result["ready"]
        .as_bool()
        .ok_or(bad("Provider account readiness missing"))?;
    // No arbitrary service payload or credential is returned to merchant/model callers.
    let mut response = json!({"provider":id,"channel":channel,"accountRef":account,"ready":ready,"environment":environment,"status":result["status"]});
    if let Some(url) = result["onboardingUrl"].as_str() {
        let p = onboarding_attempt(id, &t, &m.version, environment);
        response["onboardingUrl"] = json!(approval_url(host, &p, &json!(url))?);
    }
    let methods = result["methods"]
        .as_array()
        .filter(|ms| {
            ms.len() <= 32
                && ms.iter().all(|v| {
                    v.as_str()
                        .is_some_and(|id| contract.methods.iter().any(|method| method.id == id))
                })
        })
        .ok_or(bad("Provider enabled-method list required"))?;
    if ready && methods.is_empty() {
        return Err(bad("Ready account requires enabled methods"));
    }
    response["methods"] = json!(methods);
    let public = json!({"accountRef":account,"ready":ready,"environment":environment,"methods":methods,"status":result["status"]});
    bind_account(
        a,
        (&t, id, &m.version),
        (channel, account, environment),
        op == "start",
        (ready, public),
    )
    .await?;
    Ok(response)
}

/// The approval URL check validates against the provider identity of the onboarding call; no ledger attempt exists.
fn onboarding_attempt(provider: &str, tenant: &str, version: &str, environment: &str) -> Attempt {
    Attempt {
        id: "".into(),
        provider: provider.into(),
        context: json!({}),
        tenant: tenant.into(),
        order: "".into(),
        amount: 0,
        currency: "EUR".into(),
        currency_scale: 2,
        state: "".into(),
        provider_order: None,
        capture: None,
        refunded: 0,
        adapter_version: version.into(),
        environment: environment.into(),
        bn_code: "".into(),
    }
}

/// A concurrent uninstall/version change must not activate an obsolete account contract.
async fn bind_account(
    a: &State,
    (t, id, version): (&str, &str, &str),
    (channel, account, environment): (&str, &str, &str),
    start: bool,
    (ready, public): (bool, Value),
) -> Result<()> {
    let mut tx = a.db.begin().await?;
    let current: bool = sqlx::query_scalar(
        "SELECT active FROM app_packages WHERE tenant=$1 AND id=$2 AND version=$3 FOR SHARE",
    )
    .bind(t)
    .bind(id)
    .bind(version)
    .fetch_optional(&mut *tx)
    .await?
    .unwrap_or(false);
    if !current {
        return Err(conflict("Payment package changed during onboarding"));
    }
    let statement = if start {
        "INSERT INTO payment_provider_accounts(tenant,app,channel,account_ref,environment,ready,data) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT(tenant,app,channel) DO UPDATE SET account_ref=EXCLUDED.account_ref,environment=EXCLUDED.environment,ready=EXCLUDED.ready,data=EXCLUDED.data,updated_at=now()"
    } else {
        // Inspecting or disconnecting another environment cannot replace the active payment account.
        "UPDATE payment_provider_accounts SET ready=$6,data=$7,updated_at=now() WHERE tenant=$1 AND app=$2 AND channel=$3 AND account_ref=$4 AND environment=$5"
    };
    sqlx::query(statement)
        .bind(t)
        .bind(id)
        .bind(channel)
        .bind(account)
        .bind(environment)
        .bind(ready)
        .bind(public)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// Only public deployment metadata is exposed; service URLs and credentials remain server-side.
pub fn configured_environments(c: &Value) -> Value {
    let environments: Vec<_> = ["simulation", "sandbox", "live", "contract-fixture"]
        .into_iter()
        .filter(|env| c[*env].is_object() || (c["url"].is_string() && c["environment"] == *env))
        .collect();
    json!({"environments":environments,"defaultEnvironment":c["defaultEnvironment"].as_str().or(c["environment"].as_str()).or(environments.first().copied())})
}

/// Installed adapter providers, their channel accounts and public environment metadata.
pub async fn providers(a: &State, h: &RequestContext) -> Result<Value> {
    let t = merchant(h)?;
    permit(h, "payments.read")?;
    let rows=sqlx::query("SELECT id,version,active,manifest->'paymentProvider' AS contract FROM app_packages WHERE tenant=$1 AND manifest ? 'paymentProvider' ORDER BY id").bind(&t).fetch_all(&a.db).await?;
    let accounts=sqlx::query("SELECT app,channel,account_ref,environment,ready,updated_at::text FROM payment_provider_accounts WHERE tenant=$1 ORDER BY app,channel").bind(&t).fetch_all(&a.db).await?;
    let channels = sqlx::query("SELECT id FROM sales_channels WHERE tenant=$1 ORDER BY id")
        .bind(&t)
        .fetch_all(&a.db)
        .await?;
    let services = &process_config::get().payment_services;
    let providers: Vec<_> = rows
        .iter()
        .map(|r| {
            let (id, version) = (r.get::<String, _>("id"), r.get::<String, _>("version"));
            json!({"id":id,"version":version,"active":r.get::<bool,_>("active"),"contract":r.get::<Value,_>("contract"),"configuration":configured_environments(&services[&id][&version])})
        })
        .collect();
    Ok(
        json!({"apiVersion":"1","channels":channels.iter().map(|r|json!({"id":r.get::<String,_>("id")})).collect::<Vec<_>>(),"providers":providers,"accounts":accounts.iter().map(|r|json!({"provider":r.get::<String,_>("app"),"channel":r.get::<String,_>("channel"),"accountRef":r.get::<String,_>("account_ref"),"environment":r.get::<String,_>("environment"),"ready":r.get::<bool,_>("ready"),"updatedAt":r.get::<String,_>("updated_at")})).collect::<Vec<_>>() }),
    )
}

/// An adapter provider is configured once it is active and one of its channel accounts is ready.
pub fn mark_configured(generic: &Value) -> Vec<Value> {
    let accounts = generic["accounts"].as_array().cloned().unwrap_or_default();
    generic["providers"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| {
            let mut p = p.clone();
            p["configured"] = json!(
                p["active"] == true
                    && accounts
                        .iter()
                        .any(|c| c["provider"] == p["id"] && c["ready"] == true)
            );
            p
        })
        .collect()
}

/// The merchant ledger overview: providers, the latest attempts with settlement summary and the latest jobs.
pub async fn status(gateway: &dyn Gateway, a: &State, h: &RequestContext) -> Result<Value> {
    let t = merchant(h)?;
    permit(h, "payments.read")?;
    let rows = sqlx::query(
        "SELECT * FROM payment_attempts WHERE tenant=$1 ORDER BY created_at DESC LIMIT 100",
    )
    .bind(&t)
    .fetch_all(&a.db)
    .await?;
    let jobs=sqlx::query("SELECT id,attempt_id,operation,state,error FROM payment_jobs WHERE tenant=$1 ORDER BY created_at DESC LIMIT 100").bind(&t).fetch_all(&a.db).await?;
    let mut list = vec![gateway.builtin(&t)];
    list.extend(mark_configured(&providers(a, h).await?));
    if !list.iter().any(|p| p["id"] == "shopware_payments") {
        list.push(json!({"id":"shopware_payments","configured":false,"status":"private-adapter-required"}));
    }
    Ok(
        json!({"providers":list,"attempts":rows.iter().map(|r|json!({"provider":r.get::<String,_>("provider"),"id":r.get::<String,_>("id"),"orderId":r.get::<String,_>("order_id"),"state":r.get::<String,_>("state"),"amountMinor":r.get::<i64,_>("amount_minor"),"currency":r.get::<String,_>("currency"),"currencyScale":r.get::<i16,_>("currency_scale"),"refundedMinor":r.get::<i64,_>("refunded_minor"),"adapterVersion":r.get::<String,_>("adapter_version"),"environment":r.get::<String,_>("environment"),"settlement":summary(&attempt(r))})).collect::<Vec<_>>(),"jobs":jobs.iter().map(|r|json!({"id":r.get::<String,_>("id"),"attemptId":r.get::<String,_>("attempt_id"),"operation":r.get::<String,_>("operation"),"state":r.get::<String,_>("state"),"error":r.get::<Option<String>,_>("error")})).collect::<Vec<_>>()}),
    )
}

/// Generation and coding agents use the same private account action contract.
pub fn onboarding_action() -> Value {
    json!({"name":"onboarding","description":"Manage payment provider account","handler":"payment_onboarding","public":false,"permission":"payments.manage","mcp":true,"flowAllowed":false,"inputSchema":{"type":"object","properties":{"operation":{"type":"string","enum":["start","status","disconnect"]},"channel":{"type":"string"},"environment":{"type":"string","enum":["sandbox","live","simulation","contract-fixture"]},"country":{"type":"string"},"requestKey":{"type":"string"},"approve":{"type":"boolean"}},"required":["operation","requestKey"],"additionalProperties":false}})
}

#[cfg(test)]
mod tests {
    use super::{configured_environments, mark_configured, onboarding_action};
    use serde_json::json;

    #[test]
    fn environments_expose_only_public_metadata() {
        let c = json!({"sandbox":{"url":"https://s","token":"x"},"live":{"url":"https://l"},"defaultEnvironment":"sandbox"});
        let v = configured_environments(&c);
        assert_eq!(v["environments"], json!(["sandbox", "live"]));
        assert_eq!(v["defaultEnvironment"], "sandbox");
        assert!(!v.to_string().contains("token"));
        let single = configured_environments(&json!({"url":"https://x","environment":"live"}));
        assert_eq!(single["environments"], json!(["live"]));
        assert_eq!(
            configured_environments(&json!(null))["environments"],
            json!([])
        );
    }

    #[test]
    fn providers_are_configured_only_when_active_with_a_ready_account() {
        let generic = json!({"providers":[{"id":"a","active":true},{"id":"b","active":true},{"id":"c","active":false}],"accounts":[{"provider":"a","ready":true},{"provider":"b","ready":false},{"provider":"c","ready":true}]});
        let marked = mark_configured(&generic);
        assert_eq!(
            marked
                .iter()
                .map(|p| p["configured"] == true)
                .collect::<Vec<_>>(),
            [true, false, false]
        );
    }

    #[test]
    fn onboarding_is_private_and_requires_approval_inputs() {
        let a = onboarding_action();
        assert_eq!(a["public"], false);
        assert_eq!(a["flowAllowed"], false);
        assert_eq!(a["permission"], "payments.manage");
    }
}
