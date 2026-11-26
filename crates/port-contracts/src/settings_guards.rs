//! Contract of `veyndo_shop::ports::SettingsGuards::condition`, and a test adapter that admits a fixed set of condition types.
use serde_json::{Value, json};
use veyndo_core::{PortFuture, Result, bad};
use veyndo_shop::Settings;
use veyndo_shop::ports::SettingsGuards;

/// Every implementation admits an always-valid condition and refuses an unknown or non-string
/// type, a condition that is not an object, and an unknown type nested in a container.
pub fn condition(guards: &dyn SettingsGuards) {
    assert!(
        guards.condition(&json!({"type":"alwaysValid"})).is_ok(),
        "SettingsGuards: an always-valid condition was refused"
    );
    for refused in [
        json!({"type":"noSuchCondition"}),
        json!({"type":5}),
        json!({}),
        json!([]),
        json!("alwaysValid"),
        json!({"type":"andContainer","rules":[{"type":"noSuchCondition"}]}),
    ] {
        assert!(
            guards.condition(&refused).is_err(),
            "SettingsGuards: condition {refused} was admitted"
        );
    }
}

/// The test adapter: a condition is an object with one of `types`; containers check their rules.
pub struct Conditions {
    pub types: Vec<String>,
}

impl Default for Conditions {
    fn default() -> Self {
        Self {
            types: vec!["alwaysValid".into(), "andContainer".into()],
        }
    }
}

impl SettingsGuards for Conditions {
    fn condition(&self, v: &Value) -> Result<()> {
        let kind = v.get("type").and_then(Value::as_str).unwrap_or("");
        if !self.types.iter().any(|t| t == kind) {
            return Err(bad("Unknown condition"));
        }
        v.get("rules")
            .and_then(Value::as_array)
            .map_or(Ok(()), |rules| {
                rules.iter().try_for_each(|r| self.condition(r))
            })
    }
    fn lock<'a, 'c: 'a>(
        &'a self,
        _tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        _t: &'a str,
    ) -> PortFuture<'a, ()> {
        crate::admitted()
    }
    fn capture<'a, 'c: 'a>(
        &'a self,
        _tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        _t: &'a str,
        _settings: &'a Settings,
    ) -> PortFuture<'a, ()> {
        crate::admitted()
    }
    fn locale<'a, 'c: 'a>(
        &'a self,
        _tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        _t: &'a str,
        _locale: &'a str,
    ) -> PortFuture<'a, ()> {
        crate::admitted()
    }
}

/// No document of a component above pricing refers to a currency or bounds a price job.
impl veyndo_pricing::ports::References for Conditions {
    fn product_ceiling<'a>(
        &'a self,
        _: &'a mut sqlx::PgConnection,
        _: &'a str,
    ) -> PortFuture<'a, String> {
        Box::pin(async { Ok(String::new()) })
    }
    fn products_use<'a>(
        &'a self,
        _: &'a mut sqlx::PgConnection,
        _: &'a str,
        _: &'a [String],
    ) -> PortFuture<'a, bool> {
        Box::pin(async { Ok(false) })
    }
    fn carts_use<'a>(
        &'a self,
        _: &'a mut sqlx::PgConnection,
        _: &'a str,
        _: &'a [String],
    ) -> PortFuture<'a, bool> {
        Box::pin(async { Ok(false) })
    }
    fn units_use<'a>(
        &'a self,
        _: &'a mut sqlx::PgConnection,
        _: &'a str,
        _: &'a [String],
    ) -> PortFuture<'a, bool> {
        Box::pin(async { Ok(false) })
    }
}

/// No document refers to a shipping or payment method.
impl veyndo_shipping::ports::MethodReferences for Conditions {
    fn orders<'a>(
        &'a self,
        _: &'a mut sqlx::PgConnection,
        _: &'a veyndo_shipping::ports::MethodReference<'a>,
    ) -> PortFuture<'a, i64> {
        Box::pin(async { Ok(0) })
    }
    fn carts<'a>(
        &'a self,
        _: &'a mut sqlx::PgConnection,
        _: &'a veyndo_shipping::ports::MethodReference<'a>,
    ) -> PortFuture<'a, i64> {
        Box::pin(async { Ok(0) })
    }
    fn overrides<'a>(
        &'a self,
        _: &'a mut sqlx::PgConnection,
        _: &'a veyndo_shipping::ports::MethodReference<'a>,
    ) -> PortFuture<'a, i64> {
        Box::pin(async { Ok(0) })
    }
    fn rules<'a>(
        &'a self,
        _: &'a mut sqlx::PgConnection,
        _: &'a veyndo_shipping::ports::MethodReference<'a>,
    ) -> PortFuture<'a, i64> {
        Box::pin(async { Ok(0) })
    }
    fn units<'a>(
        &'a self,
        _: &'a mut sqlx::PgConnection,
        _: &'a veyndo_shipping::ports::MethodReference<'a>,
    ) -> PortFuture<'a, i64> {
        Box::pin(async { Ok(0) })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_test_adapter_meets_the_contract() {
        super::condition(&super::Conditions::default());
    }
}
