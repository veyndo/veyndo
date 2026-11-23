//! App-owned RFQ preparation states reuse native quote CAS, ownership, history and outbox; only native offer/accept moves money.
use serde_json::{Value, json};
use veyndo_app_model::native_lifecycle::{MachineEdge, Snapshot, reason as edge_reason};
use veyndo_app_model::native_lifecycle_snapshot::snapshot as accepted;
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad, conflict};
use veyndo_identity::permit;
use veyndo_kernel::verified_kernel;

/// The lifecycle a quote was accepted under, if it carries one.
fn snapshot(data: &Value) -> Result<Option<Snapshot>> {
    accepted(data, "quote").map_err(|e| match e.kind {
        veyndo_app_model::Kind::Bad => bad(e.message),
        veyndo_app_model::Kind::Conflict => conflict(&e.message),
        veyndo_app_model::Kind::Validation => {
            Error::validation(e.pointer.as_deref().unwrap_or(""), e.message)
        }
    })
}

/// Why a transition is refused for the current actor and facts, if it is.
fn reason(h: &RequestContext, edge: &MachineEdge, data: &Value) -> Option<&'static str> {
    edge_reason(edge, data, |p| permit(h, p).is_ok())
}
/// The state and policy an app-declared quote transition leads to, when the actor and facts admit it.
pub fn transition(
    h: &RequestContext,
    data: &Value,
    current: &str,
    action: &str,
) -> Result<(String, Value)> {
    let snapshot = snapshot(data)?.ok_or(conflict("Quote has no accepted app lifecycle"))?;
    let edge = snapshot
        .machine
        .transitions
        .iter()
        .find(|e| e.from == current && e.id == action)
        .ok_or(conflict(
            "Quote transition is not declared for its accepted version",
        ))?;
    let reason = reason(h, edge, data);
    let native = current == "requested"
        || snapshot.machine.states.iter().any(|s| {
            s.id == current && !["offered", "accepted", "ordered", "rejected"].contains(&current)
        });
    if !verified_kernel::native_lifecycle_admissible(
        native,
        reason != Some("lifecyclePermission"),
        reason != Some("lifecycleGuard"),
    ) {
        return Err(Error(
            if reason == Some("lifecyclePermission") {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::CONFLICT
            },
            reason.unwrap_or("Quote is closed").into(),
        ));
    }
    Ok((edge.to.clone(), json!(edge.policy)))
}
/// Whether the quote is still in a preparation state: requested, or an app state before the native offer.
pub fn preparation(data: &Value, state: &str) -> bool {
    state == "requested"
        || snapshot(data).ok().flatten().is_some_and(|s| {
            s.machine.states.iter().any(|s| s.id == state)
                && !["offered", "accepted", "ordered", "rejected"].contains(&state)
        })
}
/// The quote's app lifecycle as a response: its label, the available actions for a merchant and the preparation flag.
pub fn projection(data: &Value, state: &str, h: &RequestContext, admin: bool) -> Value {
    let Some(snapshot) = snapshot(data).ok().flatten() else {
        return Value::Null;
    };
    let label = snapshot
        .machine
        .states
        .iter()
        .find(|s| s.id == state)
        .map(|s| &s.label);
    let actions = if admin {
        snapshot.machine.transitions.iter().filter(|e| e.from == state).map(|e| {
        let reason = reason(h, e, data);
        json!({"id":e.id,"state":e.to,"label":e.label,"enabled":reason.is_none(),"reason":reason,"source":e.policy})
    }).collect::<Vec<_>>()
    } else {
        vec![]
    };
    json!({"label":label,"actions":actions,"mainLocale":snapshot.main_locale,"preparation":preparation(data,state)})
}

#[cfg(test)]
mod tests {
    use super::{preparation, projection, transition};
    use serde_json::{Value, json};
    use veyndo_core::RequestContext;

    /// A quote accepted under an app lifecycle with one review state before the native offer.
    fn quote() -> Value {
        let state =
            |id: &str, terminal: bool| json!({"id":id,"label":{"en-GB":id},"terminal":terminal});
        json!({"nativeLifecycle":{"version":1,"object":"quote","revision":0,"mainLocale":"en-GB","machine":{
            "states":[state("requested",false),state("review",false),state("offered",false),state("accepted",false),state("ordered",true),state("rejected",true)],
            "transitions":[{"id":"start_review","from":"requested","to":"review","label":{"en-GB":"Review"}}]}}})
    }

    #[test]
    fn declared_transitions_lead_to_their_state_and_others_are_refused() {
        let h = RequestContext::new();
        let (state, _) = transition(&h, &quote(), "requested", "start_review").unwrap();
        assert_eq!(state, "review");
        assert_eq!(
            transition(&h, &quote(), "review", "start_review")
                .err()
                .unwrap()
                .0
                .as_u16(),
            409
        );
        assert_eq!(
            transition(&h, &json!({}), "requested", "start_review")
                .err()
                .unwrap()
                .0
                .as_u16(),
            409
        );
    }

    #[test]
    fn only_requested_and_app_states_before_the_offer_are_preparation() {
        assert!(preparation(&quote(), "requested"));
        assert!(preparation(&quote(), "review"));
        assert!(!preparation(&quote(), "offered"));
        assert!(!preparation(&json!({}), "review"));
        assert!(preparation(&json!({}), "requested"));
    }

    #[test]
    fn projection_shows_actions_to_merchants_only() {
        let h = RequestContext::new();
        let merchant = projection(&quote(), "requested", &h, true);
        assert_eq!(merchant["actions"][0]["id"], "start_review");
        assert_eq!(merchant["actions"][0]["enabled"], true);
        assert_eq!(merchant["label"]["en-GB"], "requested");
        let buyer = projection(&quote(), "requested", &h, false);
        assert_eq!(buyer["actions"], json!([]));
        assert!(projection(&json!({}), "requested", &h, true).is_null());
    }
}
