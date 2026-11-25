//! Generated domain dispatcher; each bounded module calls the original production policies.
use serde::Deserialize;
use serde_json::Value;
use veyndo_core::document::{lenient, view};
mod access;
mod agents;
mod apps;
mod automation;
mod commerce;
mod fulfillment;
mod intelligence;
/// One conformance request: the policy to evaluate and its arguments.
#[derive(Deserialize, Default)]
#[serde(default)]
pub(super) struct Request {
    #[serde(deserialize_with = "lenient")]
    pub(super) function: Option<String>,
    pub(super) args: Value,
}
pub(super) fn eval(j: &Value) -> Result<Value, String> {
    let request: Request = view(j);
    match request.function.as_deref() {
        Some("auth_factor_admissible")
        | Some("channel_access_admissible")
        | Some("consent_admissible")
        | Some("legal_checkout_admissible")
        | Some("merchant_scope_admissible")
        | Some("native_asset_bypass")
        | Some("persistent_key_admissible")
        | Some("platform_admissible")
        | Some("scope_admissible")
        | Some("shop_request_admissible")
        | Some("workspace_length_admissible") => access::eval(&request),
        Some("agent_preview_admissible") | Some("planning_request_admissible") => {
            agents::eval(&request)
        }
        Some("app_callback_admissible")
        | Some("app_configuration_release_admissible")
        | Some("app_core_reference_admissible")
        | Some("app_flow_admissible")
        | Some("app_package_authorized")
        | Some("app_read_admissible")
        | Some("app_service_transport_admissible")
        | Some("app_submission_admissible")
        | Some("app_surface_admissible")
        | Some("app_tool_admissible")
        | Some("job_completion_admissible")
        | Some("native_lifecycle_admissible")
        | Some("source_activation_admissible")
        | Some("wasm_resources_admissible") => apps::eval(&request),
        Some("customer_group_net")
        | Some("destination_tax_admissible")
        | Some("flow_delay_admissible")
        | Some("notification_retry_admissible")
        | Some("resource_quota_admissible")
        | Some("rule_authenticated")
        | Some("rule_boolean_comparison")
        | Some("rule_xor_count") => automation::eval(&request),
        Some("admin_payment_admissible")
        | Some("amount_sum")
        | Some("b2b_approval_admissible")
        | Some("b2b_budget_admissible")
        | Some("capture_admissible")
        | Some("checkout_contact_admissible")
        | Some("checkout_review_admissible")
        | Some("currency_context_admissible")
        | Some("currency_scale_admissible")
        | Some("discount_cap")
        | Some("external_amount_admissible")
        | Some("external_paid_total_admissible")
        | Some("net_amount")
        | Some("offer_honoured")
        | Some("pos_cash_admissible")
        | Some("pos_discount_admissible")
        | Some("refund_admissible")
        | Some("replay_admissible")
        | Some("reservation_admissible")
        | Some("revision_admissible")
        | Some("stock_admissible") => commerce::eval(&request),
        Some("cancellation_admissible")
        | Some("carrier_booking_admissible")
        | Some("completion_admissible")
        | Some("delivery_split_admissible")
        | Some("download_admissible")
        | Some("manual_payment_admissible")
        | Some("order_edit_admissible")
        | Some("order_transition_admissible")
        | Some("payment_transition_admissible")
        | Some("receipt_admissible")
        | Some("reservation_release_admissible") => fulfillment::eval(&request),
        Some("ai_autonomy_admissible")
        | Some("ai_price_admissible")
        | Some("claim_render_admissible")
        | Some("experiment_result_admissible")
        | Some("sovereign_request_admissible") => intelligence::eval(&request),
        _ => Err("Unknown policy".into()),
    }
}
