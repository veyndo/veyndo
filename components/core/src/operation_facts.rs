//! Typed planner facts of agent-facing operations: permission, cost, reversibility, risk, data and side effects; facts never grant rights.
//!
//! Every native operation, Flow action and protocol capability declares one `OperationFacts`
//! value. Transports, planners and Studio read them; admission and domain owners stay the
//! authority. `check` holds the invariants every declaration must satisfy, so an entry
//! that claims a cheap, reversible read while moving money is rejected before it ships.
use serde::Serialize;

/// Who may be admitted to call an operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Audience {
    /// Storefront callers: anonymous visitors, customer sessions and buying agents.
    Shopper,
    /// A signed-in merchant member holding the declared permissions.
    Merchant,
}

/// How a completed call can be undone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Reversibility {
    /// Saving the previous values again restores the prior state; nothing outside the shop changed.
    Reversible,
    /// A different named operation undoes the effect.
    Compensatable {
        /// The operation that compensates this one.
        by: &'static str,
    },
    /// The effect lasts: money moved, a message left, a record was deleted or a legal fact recorded.
    Irreversible,
}

/// The review weight of a call.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Risk {
    Low,
    Medium,
    High,
}

/// The resource class a call consumes; derived from its side effects so it cannot drift from them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Cost {
    /// A bounded local read.
    Free,
    /// A bounded local transaction.
    Standard,
    /// Queues bounded background work.
    Batch,
    /// Calls an external provider or service.
    Provider,
    /// Runs central model inference and counts against agent allowances.
    Model,
}

/// The business records a call reads or changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Entity {
    Product,
    Category,
    Media,
    Order,
    Customer,
    Cart,
    Payment,
    Settings,
    Currency,
    Legal,
    Knowledge,
    Experiment,
    Automation,
    App,
    Role,
    Run,
    Workspace,
    History,
    Company,
    Quote,
    List,
    Document,
    Report,
    Site,
    Import,
    Operation,
}

/// Effects a call has beyond its own records.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SideEffect {
    /// Sends or queues an email to a person.
    Email,
    /// Moves money through a payment provider.
    Payment,
    /// Calls an external provider or service.
    ExternalCall,
    /// Runs central model inference.
    ModelCall,
    /// Makes content visible to shoppers or the public.
    Publication,
    /// Queues durable background work.
    BackgroundJob,
    /// Emits a domain event that Flows and apps may react to.
    Event,
}

/// A state the caller must be in, beyond permissions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Condition {
    /// Only in a private sandbox, never against the live shop.
    Sandbox,
    /// Only with a personal login, never with an integration or agent credential.
    PersonalSession,
}

/// What a call touches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Data {
    pub entities: &'static [Entity],
    pub pii: bool,
    pub money: bool,
    pub stock: bool,
}

/// The typed facts of one operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationFacts {
    pub audience: Audience,
    /// Every permission admission requires together; empty for shopper calls and any signed-in member.
    pub permissions: &'static [&'static str],
    /// Whether the call changes state; a conservative mutating transport hint may still apply to reads.
    pub writes: bool,
    pub reversibility: Reversibility,
    pub risk: Risk,
    pub data: Data,
    pub effects: &'static [SideEffect],
    /// Operations whose result this one consumes: the edges of the operations graph.
    pub requires: &'static [&'static str],
    pub conditions: &'static [Condition],
}

const NOTHING: Data = Data {
    entities: &[],
    pii: false,
    money: false,
    stock: false,
};

impl OperationFacts {
    /// A call that changes nothing; reads start at low risk.
    pub const fn read(audience: Audience, permissions: &'static [&'static str]) -> Self {
        Self {
            audience,
            permissions,
            writes: false,
            reversibility: Reversibility::Reversible,
            risk: Risk::Low,
            data: NOTHING,
            effects: &[],
            requires: &[],
            conditions: &[],
        }
    }
    /// A call that changes state with the given undo path and review weight.
    pub const fn write(
        audience: Audience,
        permissions: &'static [&'static str],
        reversibility: Reversibility,
        risk: Risk,
    ) -> Self {
        Self {
            writes: true,
            reversibility,
            risk,
            ..Self::read(audience, permissions)
        }
    }
    pub const fn on(mut self, entities: &'static [Entity]) -> Self {
        self.data.entities = entities;
        self
    }
    /// Personal data: reads and writes are at least medium risk.
    pub const fn pii(mut self) -> Self {
        self.data.pii = true;
        if matches!(self.risk, Risk::Low) {
            self.risk = Risk::Medium;
        }
        self
    }
    pub const fn money(mut self) -> Self {
        self.data.money = true;
        self
    }
    pub const fn stock(mut self) -> Self {
        self.data.stock = true;
        self
    }
    pub const fn effects(mut self, effects: &'static [SideEffect]) -> Self {
        self.effects = effects;
        self
    }
    pub const fn requires(mut self, operations: &'static [&'static str]) -> Self {
        self.requires = operations;
        self
    }
    pub const fn when(mut self, conditions: &'static [Condition]) -> Self {
        self.conditions = conditions;
        self
    }
    /// The resource class follows from the declared effects.
    pub fn cost(&self) -> Cost {
        let has = |effect| self.effects.contains(&effect);
        if has(SideEffect::ModelCall) {
            Cost::Model
        } else if has(SideEffect::ExternalCall) || has(SideEffect::Payment) {
            Cost::Provider
        } else if has(SideEffect::BackgroundJob) {
            Cost::Batch
        } else if self.writes {
            Cost::Standard
        } else {
            Cost::Free
        }
    }
    /// A high-risk or irreversible change needs a person's approval through the existing controls.
    pub fn requires_approval(&self) -> bool {
        self.writes
            && (self.risk == Risk::High || self.reversibility == Reversibility::Irreversible)
    }
    /// The compensating operation, if one exists.
    pub fn compensation(&self) -> Option<&'static str> {
        match self.reversibility {
            Reversibility::Compensatable { by } => Some(by),
            _ => None,
        }
    }
    /// Invariants every declaration satisfies; names are checked against the registry by its owner.
    pub fn check(&self, name: &str) -> std::result::Result<(), String> {
        let fail = |why: &str| Err(format!("{name}: {why}"));
        let has = |effect| self.effects.contains(&effect);
        if !self.writes {
            if self.reversibility != Reversibility::Reversible {
                return fail("a read is trivially reversible");
            }
            if self.risk == Risk::High {
                return fail("a read cannot be high risk; it changes nothing");
            }
            if [
                SideEffect::Email,
                SideEffect::Payment,
                SideEffect::Publication,
                SideEffect::BackgroundJob,
                SideEffect::Event,
            ]
            .into_iter()
            .any(has)
            {
                return fail("a read cannot send, pay, publish, queue or emit");
            }
        }
        if self.reversibility == Reversibility::Irreversible && self.risk == Risk::Low {
            return fail("an irreversible change is at least medium risk");
        }
        if (has(SideEffect::Email) || has(SideEffect::Payment))
            && self.reversibility == Reversibility::Reversible
        {
            return fail("a sent email or moved money cannot be undone by saving again");
        }
        if has(SideEffect::Payment) && self.risk != Risk::High {
            return fail("moving money is high risk");
        }
        if self.data.pii && self.risk == Risk::Low {
            return fail("personal data is at least medium risk");
        }
        if self.compensation() == Some(name) {
            return fail("an operation cannot compensate itself");
        }
        if self.requires.contains(&name) {
            return fail("an operation cannot require itself");
        }
        if self.audience == Audience::Shopper && !self.permissions.is_empty() {
            return fail("shopper calls are admitted by session, not merchant permissions");
        }
        let mut seen = std::collections::HashSet::new();
        if !self
            .permissions
            .iter()
            .all(|permission| seen.insert(permission))
        {
            return fail("permissions are listed once");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Audience, Cost, Entity, OperationFacts, Reversibility, Risk, SideEffect};

    const READ: OperationFacts = OperationFacts::read(Audience::Merchant, &["orders.read"]);

    #[test]
    fn cost_and_approval_follow_from_effects_risk_and_reversibility() {
        assert_eq!(READ.cost(), Cost::Free);
        assert!(!READ.requires_approval());
        let plan = READ.effects(&[SideEffect::ModelCall]);
        assert_eq!(plan.cost(), Cost::Model);
        let pay = OperationFacts::write(
            Audience::Merchant,
            &["payments.manage"],
            Reversibility::Irreversible,
            Risk::High,
        )
        .effects(&[SideEffect::Payment]);
        assert_eq!(pay.cost(), Cost::Provider);
        assert!(pay.requires_approval());
        assert!(pay.check("merchant.payment").is_ok());
        let save = OperationFacts::write(
            Audience::Merchant,
            &["catalog.write"],
            Reversibility::Compensatable {
                by: "merchant.history.restore",
            },
            Risk::Medium,
        )
        .on(&[Entity::Product]);
        assert_eq!(save.cost(), Cost::Standard);
        assert_eq!(save.compensation(), Some("merchant.history.restore"));
        assert!(!save.requires_approval());
        assert_eq!(READ.on(&[Entity::Customer]).pii().risk, Risk::Medium);
    }

    #[test]
    fn contradictory_declarations_are_rejected() {
        let write = |reversibility, risk| {
            OperationFacts::write(Audience::Merchant, &["orders.write"], reversibility, risk)
        };
        for (facts, why) in [
            (READ.effects(&[SideEffect::Email]), "a read cannot send"),
            (
                OperationFacts {
                    risk: Risk::High,
                    ..READ
                },
                "a read cannot be high",
            ),
            (
                OperationFacts {
                    reversibility: Reversibility::Irreversible,
                    ..READ
                },
                "trivially reversible",
            ),
            (
                write(Reversibility::Irreversible, Risk::Low),
                "at least medium",
            ),
            (
                write(Reversibility::Reversible, Risk::High).effects(&[SideEffect::Payment]),
                "cannot be undone",
            ),
            (
                write(Reversibility::Irreversible, Risk::Medium).effects(&[SideEffect::Payment]),
                "moving money is high",
            ),
            (
                OperationFacts {
                    data: super::Data {
                        pii: true,
                        ..READ.data
                    },
                    ..READ
                },
                "personal data",
            ),
            (
                write(Reversibility::Compensatable { by: "x" }, Risk::Low),
                "compensate itself",
            ),
            (READ.requires(&["x"]), "require itself"),
            (
                OperationFacts::read(Audience::Shopper, &["orders.read"]),
                "shopper calls",
            ),
            (
                OperationFacts::read(Audience::Merchant, &["orders.read", "orders.read"]),
                "listed once",
            ),
        ] {
            let error = facts.check("x").unwrap_err();
            assert!(error.contains(why), "{error}");
        }
    }

    #[test]
    fn facts_serialize_as_the_planner_reads_them() {
        let facts = OperationFacts::write(
            Audience::Merchant,
            &["settings.write"],
            Reversibility::Compensatable {
                by: "knowledge.autonomy.revert",
            },
            Risk::High,
        )
        .on(&[Entity::Product])
        .money();
        assert_eq!(
            serde_json::to_value(facts).unwrap(),
            serde_json::json!({"audience":"merchant","permissions":["settings.write"],"writes":true,
            "reversibility":{"kind":"compensatable","by":"knowledge.autonomy.revert"},"risk":"high",
            "data":{"entities":["product"],"pii":false,"money":true,"stock":false},"effects":[],
            "requires":[],"conditions":[]})
        );
    }
}
