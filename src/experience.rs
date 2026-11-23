//! The storefront experience lives in the intelligence component; this handler supplies the experiment assignment secret.
use crate::*;
pub(crate) use veyndo_intelligence::http::experience::{
    forget_personalization, personalization, policy_stats,
};
pub(crate) async fn experience(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let secret = env::var("PLATFORM_SECRET_KEY").unwrap_or_else(|_| a.token.to_string());
    veyndo_intelligence::http::experience::experience(&a, &secret, h, v).await
}
