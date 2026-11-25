//! Merchant readiness reports native implementation and current channel configuration, never external programme approval or settlement.
use crate::foundation::App;
use crate::foundation::merchant;
use crate::{auth, commerce, marketing};
use crate::{cognition, mcp};
use serde_json::Value;
use serde_json::json;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
pub(crate) async fn readiness(a: &App, h: &RequestContext) -> Result<Value> {
    let t = merchant(a, h)?;
    auth::permit(h, "catalog.read")?;
    let channel = marketing::channel_id(h);
    let (settings, revision) = commerce::scoped_config(a, &t, channel).await?;
    marketing::channel(a, h, channel, &settings.main_locale).await?;
    Ok(
        json!({"destination":"veyndo","channel":channel,"settingsRevision":revision,"cartTransfer":true,"buyerConsent":true,"poppy":{"published":mcp::poppy::published_url(&t).is_some(),"url":mcp::poppy::published_url(&t),"version":"1.0","offers":["mcp"]},"signedOffers":{"configured":cognition::signed::key_id().is_some(),"keyId":cognition::signed::key_id(),"validSeconds":600,"offerPath":"/store-api/checkout/cart/offer","verifyPath":"/store-api/checkout/offer/verify","keysPath":"/.well-known/commerce-facts.json"},"externalAcceptance":"not_verified","private":veyndo_core::store::environments::parent(a,&t).await?.is_some(),"configuredPayments":settings.payments.iter().filter(|p|p.active).map(|p|json!({"id":p.id,"mode":p.mode})).collect::<Vec<_>>(),"profilePath":"/.well-known/ucp","cartPath":"/ucp/v1/carts","inboundSources":["csv","shopify_csv","shop_mcp","url"]}),
    )
}
