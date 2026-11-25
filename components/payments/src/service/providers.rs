//! Adapter providers of a shop: merchant-approved account onboarding, the public provider listing and the ledger status overview; only authenticated provider responses establish account bindings.
use crate::domain::attempt::Attempt;
use crate::domain::service_config::approval_url;
use crate::domain::settlements::summary;
use crate::ports::{Gateway, Host};
use crate::store::{attempts, jobs, provider_accounts};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::{
    Error, RequestContext, Result, State, StatusCode, bad, conflict, process_config,
};
use veyndo_identity::{merchant, permit};

/// The JSON fields `onboarding` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct OnboardingInput {
    #[serde(deserialize_with = "lenient")]
    operation: Option<String>,
    #[serde(deserialize_with = "lenient")]
    approve: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    channel: Option<String>,
    #[serde(deserialize_with = "lenient")]
    environment: Option<String>,
    #[serde(deserialize_with = "lenient")]
    request_key: Option<String>,
    country: Value,
}

/// Starts, inspects or disconnects the provider account of one sales channel; every effect needs `approve`.
pub async fn onboarding(
    (host, gateway): (&dyn Host, &dyn Gateway),
    a: &State,
    h: &RequestContext,
    id: &str,
    input: &Value,
) -> Result<Value> {
    let args: OnboardingInput = view(input);
    let t = merchant(h)?;
    permit(h, "payments.manage")?;
    let m = gateway.package(&t, id).await?;
    let contract = m
        .payment_provider
        .as_ref()
        .ok_or(bad("App has no payment provider contract"))?;
    let op = args.operation.as_deref().unwrap_or("status");
    if !["status", "start", "disconnect"].contains(&op)
        || (op != "status" && args.approve != Some(true))
    {
        return Err(bad("Supported onboarding operation and approval required"));
    }
    let channel = args.channel.as_deref().unwrap_or("default");
    if !provider_accounts::channel_exists(&a.db, &t, channel).await? {
        return Err(Error(
            StatusCode::NOT_FOUND,
            "Sales channel not found".into(),
        ));
    }
    let configured = &process_config::get().payment_services[id][&m.version];
    let configured_service: ConfiguredService = view(configured);
    let environment = args
        .environment
        .as_deref()
        .or_else(|| configured_service.default_environment.as_str())
        .or_else(|| configured_service.environment.as_str())
        .ok_or(Error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Payment service not configured".into(),
        ))?;
    let key = args
        .request_key
        .as_deref()
        .filter(|v| v.len() >= 8 && v.len() <= 128)
        .ok_or(bad("Onboarding requestKey required"))?;
    let body = json!({"apiVersion":"1","provider":id,"adapterVersion":m.version,"tenant":t,"channel":channel,"environment":environment,"operation":op,"requestKey":key,"country":args.country});
    let result = gateway
        .call(&t, id, &m.version, environment, "onboarding", &body)
        .await?;
    let receipt: OnboardingReceipt = view(&result);
    if receipt.api_version.as_deref() != Some("1")
        || receipt.adapter_version.as_deref() != Some(m.version.as_str())
        || receipt.tenant.as_deref() != Some(t.as_str())
        || receipt.provider.as_deref() != Some(id)
        || receipt.environment.as_deref() != Some(environment)
        || receipt.channel.as_deref() != Some(channel)
    {
        return Err(bad("Onboarding account identity mismatch"));
    }
    let account = receipt
        .account_ref
        .as_deref()
        .filter(|v| !v.is_empty() && v.len() <= 200)
        .ok_or(bad("Provider account reference missing"))?;
    let ready = receipt
        .ready
        .ok_or(bad("Provider account readiness missing"))?;
    // No arbitrary service payload or credential is returned to merchant/model callers.
    let mut response = json!({"provider":id,"channel":channel,"accountRef":account,"ready":ready,"environment":environment,"status":receipt.status});
    if let Some(url) = &receipt.onboarding_url {
        let p = onboarding_attempt(id, &t, &m.version, environment);
        response["onboardingUrl"] = json!(approval_url(host, &p, &json!(url))?);
    }
    let methods = receipt
        .methods
        .as_ref()
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
    let public = json!({"accountRef":account,"ready":ready,"environment":environment,"methods":methods,"status":receipt.status});
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

/// The adapter's onboarding answer; each field of another shape reads as absent, as indexing did.
#[derive(Deserialize, Default, Debug, PartialEq)]
#[serde(default, rename_all = "camelCase")]
struct OnboardingReceipt {
    #[serde(deserialize_with = "lenient")]
    api_version: Option<String>,
    #[serde(deserialize_with = "lenient")]
    adapter_version: Option<String>,
    #[serde(deserialize_with = "lenient")]
    tenant: Option<String>,
    #[serde(deserialize_with = "lenient")]
    provider: Option<String>,
    #[serde(deserialize_with = "lenient")]
    environment: Option<String>,
    #[serde(deserialize_with = "lenient")]
    channel: Option<String>,
    #[serde(deserialize_with = "lenient")]
    account_ref: Option<String>,
    #[serde(deserialize_with = "lenient")]
    ready: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    onboarding_url: Option<String>,
    #[serde(deserialize_with = "lenient")]
    methods: Option<Vec<Value>>,
    status: Value,
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
    if !provider_accounts::share_package_active(&mut tx, t, id, version).await? {
        return Err(conflict("Payment package changed during onboarding"));
    }
    let key = provider_accounts::AccountKey {
        tenant: t,
        app: id,
        channel,
        account_ref: account,
        environment,
    };
    if start {
        provider_accounts::bind(&mut tx, &key, ready, &public).await?;
    } else {
        // Inspecting or disconnecting another environment cannot replace the active payment account.
        provider_accounts::refresh(&mut tx, &key, ready, &public).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// The environments of a configured adapter service version.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ConfiguredService {
    default_environment: Value,
    environment: Value,
}

/// The JSON fields `configured_environments` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ConfiguredEnvironmentsInput {
    url: Value,
    environment: Value,
    #[serde(deserialize_with = "lenient")]
    default_environment: Option<String>,
}

/// Only public deployment metadata is exposed; service URLs and credentials remain server-side.
pub fn configured_environments(c: &Value) -> Value {
    let input: ConfiguredEnvironmentsInput = view(c);
    let environments: Vec<_> = ["simulation", "sandbox", "live", "contract-fixture"]
        .into_iter()
        .filter(|env| c[*env].is_object() || (input.url.is_string() && input.environment == *env))
        .collect();
    json!({"environments":environments,"defaultEnvironment":input.default_environment.as_deref().or(input.environment.as_str()).or(environments.first().copied())})
}

/// Installed adapter providers, their channel accounts and public environment metadata.
pub async fn providers(a: &State, h: &RequestContext) -> Result<Value> {
    let t = merchant(h)?;
    permit(h, "payments.read")?;
    let rows = provider_accounts::packages(&a.db, &t).await?;
    let accounts = provider_accounts::accounts(&a.db, &t).await?;
    let channels = provider_accounts::channels(&a.db, &t).await?;
    let services = &process_config::get().payment_services;
    let providers: Vec<_> = rows
        .iter()
        .map(|r| {
            json!({"id":r.id,"version":r.version,"active":r.active,"contract":r.contract,"configuration":configured_environments(&services[&r.id][&r.version])})
        })
        .collect();
    Ok(
        json!({"apiVersion":"1","channels":channels.iter().map(|id|json!({"id":id})).collect::<Vec<_>>(),"providers":providers,"accounts":accounts.iter().map(|r|json!({"provider":r.app,"channel":r.channel,"accountRef":r.account_ref,"environment":r.environment,"ready":r.ready,"updatedAt":r.updated_at})).collect::<Vec<_>>() }),
    )
}

/// The JSON fields `mark_configured` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct MarkConfiguredInput {
    accounts: Value,
    providers: Value,
}

/// The fields of a listed provider or channel account that decide whether a provider is configured.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Listed {
    id: Value,
    active: Value,
    provider: Value,
    ready: Value,
}

/// An adapter provider is configured once it is active and one of its channel accounts is ready.
pub fn mark_configured(generic: &Value) -> Vec<Value> {
    let input: MarkConfiguredInput = view(generic);
    let accounts = input.accounts.as_array().cloned().unwrap_or_default();
    input
        .providers
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| {
            let mut p = p.clone();
            let listed: Listed = view(&p);
            p["configured"] = json!(
                listed.active == true
                    && accounts
                        .iter()
                        .map(view::<Listed>)
                        .any(|c| c.provider == listed.id && c.ready == true)
            );
            p
        })
        .collect()
}

/// The merchant ledger overview: providers, the latest attempts with settlement summary and the latest jobs.
pub async fn status(gateway: &dyn Gateway, a: &State, h: &RequestContext) -> Result<Value> {
    let t = merchant(h)?;
    permit(h, "payments.read")?;
    let rows = attempts::latest(&a.db, &t).await?;
    let jobs = jobs::latest(&a.db, &t).await?;
    let mut list = vec![gateway.builtin(&t)];
    list.extend(mark_configured(&providers(a, h).await?));
    if !list
        .iter()
        .any(|p| view::<Listed>(p).id == "shopware_payments")
    {
        list.push(json!({"id":"shopware_payments","configured":false,"status":"private-adapter-required"}));
    }
    Ok(
        json!({"providers":list,"attempts":rows.iter().map(|r|json!({"provider":r.provider,"id":r.id,"orderId":r.order,"state":r.state,"amountMinor":r.amount,"currency":r.currency,"currencyScale":r.currency_scale,"refundedMinor":r.refunded,"adapterVersion":r.adapter_version,"environment":r.environment,"settlement":summary(r)})).collect::<Vec<_>>(),"jobs":jobs.iter().map(|r|json!({"id":r.id,"attemptId":r.attempt_id,"operation":r.operation,"state":r.state,"error":r.error})).collect::<Vec<_>>()}),
    )
}

/// Generation and coding agents use the same private account action contract.
pub fn onboarding_action() -> Value {
    json!({"name":"onboarding","description":"Manage payment provider account","handler":"payment_onboarding","public":false,"permission":"payments.manage","mcp":true,"flowAllowed":false,"inputSchema":{"type":"object","properties":{"operation":{"type":"string","enum":["start","status","disconnect"]},"channel":{"type":"string"},"environment":{"type":"string","enum":["sandbox","live","simulation","contract-fixture"]},"country":{"type":"string"},"requestKey":{"type":"string"},"approve":{"type":"boolean"}},"required":["operation","requestKey"],"additionalProperties":false}})
}

#[cfg(test)]
mod tests {
    use super::{OnboardingReceipt, configured_environments, mark_configured, onboarding_action};
    use serde_json::{Value, json};
    use veyndo_core::document::view;

    #[test]
    fn the_typed_receipt_reads_exactly_what_indexing_read() {
        let legacy = |r: &Value| OnboardingReceipt {
            api_version: r["apiVersion"].as_str().map(str::to_string),
            adapter_version: r["adapterVersion"].as_str().map(str::to_string),
            tenant: r["tenant"].as_str().map(str::to_string),
            provider: r["provider"].as_str().map(str::to_string),
            environment: r["environment"].as_str().map(str::to_string),
            channel: r["channel"].as_str().map(str::to_string),
            account_ref: r["accountRef"].as_str().map(str::to_string),
            ready: r["ready"].as_bool(),
            onboarding_url: r["onboardingUrl"].as_str().map(str::to_string),
            methods: r["methods"].as_array().cloned(),
            status: r["status"].clone(),
        };
        for r in [
            json!({"apiVersion":"1","adapterVersion":"1.0.0","tenant":"s","provider":"p","environment":"sandbox","channel":"default","accountRef":"acct","ready":true,"methods":["card"],"status":{"kyc":"ok"},"onboardingUrl":"https://x"}),
            json!({"apiVersion":1,"ready":"yes","methods":"card","accountRef":7,"status":null,"onboardingUrl":false}),
            json!({"methods":[1,"card",null],"ready":false}),
            json!({}),
            json!(null),
            json!(["not", "an", "object"]),
        ] {
            assert_eq!(view::<OnboardingReceipt>(&r), legacy(&r), "{r}");
        }
    }

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
