//! Planner facts of installed app tools in the exact wire shape of the native `OperationFacts` (audience, permissions, writes, reversibility, risk, data, effects, requires, conditions, with derived cost and approval); owned because they come from package data, and they never grant rights.
use serde::{Deserialize, Serialize};

/// Who may be admitted to call an operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Audience {
    Shopper,
    Merchant,
}
/// How a completed call can be undone.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Reversibility {
    Reversible,
    /// A different named operation undoes the effect.
    Compensatable {
        by: String,
    },
    Irreversible,
}
/// The review weight of a call.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Risk {
    Low,
    Medium,
    High,
}
/// The resource class a call consumes, derived from its effects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Cost {
    Free,
    Standard,
    Batch,
    Provider,
    Model,
}
/// The business records a call reads or changes (the native vocabulary).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SideEffect {
    Email,
    Payment,
    ExternalCall,
    ModelCall,
    Publication,
    BackgroundJob,
    Event,
}
/// A state the caller must be in, beyond permissions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Condition {
    Sandbox,
    PersonalSession,
}
/// What a call touches.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Data {
    pub entities: Vec<Entity>,
    pub pii: bool,
    pub money: bool,
    pub stock: bool,
}
/// The facts of one app tool.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppFacts {
    pub audience: Audience,
    pub permissions: Vec<String>,
    pub writes: bool,
    pub reversibility: Reversibility,
    pub risk: Risk,
    pub data: Data,
    pub effects: Vec<SideEffect>,
    pub requires: Vec<String>,
    pub conditions: Vec<Condition>,
}
impl AppFacts {
    /// The resource class follows from the declared effects, as for native operations.
    pub fn cost(&self) -> Cost {
        let has = |e| self.effects.contains(&e);
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
    /// A high-risk or irreversible change needs a person's approval, as for native operations.
    pub fn requires_approval(&self) -> bool {
        self.writes
            && (self.risk == Risk::High || self.reversibility == Reversibility::Irreversible)
    }
    /// Raises the declaration to the native invariants instead of rejecting it: a declaration can
    /// only make a tool look more careful, never safer than these rules allow.
    pub fn normalize(mut self, name: &str) -> Self {
        let has = |f: &Self, e| f.effects.contains(&e);
        if let Reversibility::Compensatable { by } = &self.reversibility
            && by == name
        {
            self.reversibility = Reversibility::Irreversible;
        }
        if (has(&self, SideEffect::Email) || has(&self, SideEffect::Payment))
            && self.reversibility == Reversibility::Reversible
        {
            self.reversibility = Reversibility::Irreversible;
        }
        if has(&self, SideEffect::Payment) {
            self.risk = Risk::High;
            self.data.money = true;
        }
        if self.reversibility == Reversibility::Irreversible || self.data.pii {
            self.risk = self.risk.max(Risk::Medium);
        }
        self.requires.retain(|r| r != name);
        self.effects.sort_by_key(|e| *e as u8);
        self.effects.dedup();
        let mut seen = std::collections::HashSet::new();
        self.permissions.retain(|p| seen.insert(p.clone()));
        if self.audience == Audience::Shopper {
            self.permissions.clear();
        }
        self
    }
    /// The same invariants the native registry enforces; `normalize` output always passes.
    pub fn check(&self, name: &str) -> std::result::Result<(), String> {
        let fail = |why: &str| Err(format!("{name}: {why}"));
        let has = |e| self.effects.contains(&e);
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
        if matches!(&self.reversibility, Reversibility::Compensatable { by } if by == name) {
            return fail("an operation cannot compensate itself");
        }
        if self.requires.iter().any(|r| r == name) {
            return fail("an operation cannot require itself");
        }
        if self.audience == Audience::Shopper && !self.permissions.is_empty() {
            return fail("shopper calls are admitted by session, not merchant permissions");
        }
        Ok(())
    }
}
/// The wire members a tool description carries, as the native operation export does.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Described<'a> {
    pub facts: &'a AppFacts,
    pub cost: Cost,
    pub requires_approval: bool,
    /// Whether the publisher described the tool; otherwise these are conservative defaults.
    pub declared: bool,
}
impl<'a> Described<'a> {
    pub fn of(facts: &'a AppFacts, declared: bool) -> Self {
        Self {
            facts,
            cost: facts.cost(),
            requires_approval: facts.requires_approval(),
            declared,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every variant of the native fact vocabulary has a twin here; an added core variant must be added to both.
    #[test]
    fn app_fact_vocabulary_matches_the_native_operation_facts() {
        use veyndo_core::operation_facts as core;
        fn round<T: serde::de::DeserializeOwned>(value: impl serde::Serialize) -> T {
            serde_json::from_value(serde_json::to_value(value).unwrap()).unwrap()
        }
        let entities = [
            core::Entity::Product,
            core::Entity::Category,
            core::Entity::Media,
            core::Entity::Order,
            core::Entity::Customer,
            core::Entity::Cart,
            core::Entity::Payment,
            core::Entity::Settings,
            core::Entity::Currency,
            core::Entity::Legal,
            core::Entity::Knowledge,
            core::Entity::Experiment,
            core::Entity::Automation,
            core::Entity::App,
            core::Entity::Role,
            core::Entity::Run,
            core::Entity::Workspace,
            core::Entity::History,
            core::Entity::Company,
            core::Entity::Quote,
            core::Entity::List,
            core::Entity::Document,
            core::Entity::Report,
            core::Entity::Site,
            core::Entity::Import,
            core::Entity::Operation,
        ];
        for entity in entities {
            // Exhaustive: a new core entity fails to compile here until the list above and the twin include it.
            match entity {
                core::Entity::Product
                | core::Entity::Category
                | core::Entity::Media
                | core::Entity::Order
                | core::Entity::Customer
                | core::Entity::Cart
                | core::Entity::Payment
                | core::Entity::Settings
                | core::Entity::Currency
                | core::Entity::Legal
                | core::Entity::Knowledge
                | core::Entity::Experiment
                | core::Entity::Automation
                | core::Entity::App
                | core::Entity::Role
                | core::Entity::Run
                | core::Entity::Workspace
                | core::Entity::History
                | core::Entity::Company
                | core::Entity::Quote
                | core::Entity::List
                | core::Entity::Document
                | core::Entity::Report
                | core::Entity::Site
                | core::Entity::Import
                | core::Entity::Operation => {}
            }
            let _: Entity = round(entity);
        }
        for effect in [
            core::SideEffect::Email,
            core::SideEffect::Payment,
            core::SideEffect::ExternalCall,
            core::SideEffect::ModelCall,
            core::SideEffect::Publication,
            core::SideEffect::BackgroundJob,
            core::SideEffect::Event,
        ] {
            match effect {
                core::SideEffect::Email
                | core::SideEffect::Payment
                | core::SideEffect::ExternalCall
                | core::SideEffect::ModelCall
                | core::SideEffect::Publication
                | core::SideEffect::BackgroundJob
                | core::SideEffect::Event => {}
            }
            let _: SideEffect = round(effect);
        }
        for condition in [core::Condition::Sandbox, core::Condition::PersonalSession] {
            match condition {
                core::Condition::Sandbox | core::Condition::PersonalSession => {}
            }
            let _: Condition = round(condition);
        }
        for risk in [core::Risk::Low, core::Risk::Medium, core::Risk::High] {
            match risk {
                core::Risk::Low | core::Risk::Medium | core::Risk::High => {}
            }
            let _: Risk = round(risk);
        }
        for audience in [core::Audience::Shopper, core::Audience::Merchant] {
            match audience {
                core::Audience::Shopper | core::Audience::Merchant => {}
            }
            let _: Audience = round(audience);
        }
    }
    fn write(rev: Reversibility, risk: Risk, effects: Vec<SideEffect>) -> AppFacts {
        AppFacts {
            audience: Audience::Merchant,
            permissions: vec!["catalog".into(), "catalog".into()],
            writes: true,
            reversibility: rev,
            risk,
            data: Data::default(),
            effects,
            requires: vec!["app.a.send".into()],
            conditions: vec![],
        }
    }
    #[test]
    fn normalizing_raises_a_declaration_to_the_native_invariants() {
        let pay = write(
            Reversibility::Reversible,
            Risk::Low,
            vec![SideEffect::Payment],
        )
        .normalize("app.a.send");
        assert_eq!(
            (pay.risk, pay.reversibility.clone()),
            (Risk::High, Reversibility::Irreversible)
        );
        assert!(pay.data.money && pay.requires.is_empty() && pay.permissions == ["catalog"]);
        assert!(pay.check("app.a.send").is_ok() && pay.requires_approval());
        assert_eq!(pay.cost(), Cost::Provider);
        let own = write(
            Reversibility::Compensatable {
                by: "app.a.send".into(),
            },
            Risk::Low,
            vec![],
        )
        .normalize("app.a.send");
        assert_eq!(own.reversibility, Reversibility::Irreversible);
        assert!(own.check("app.a.send").is_ok());
        let save = write(Reversibility::Reversible, Risk::Medium, vec![]).normalize("app.a.save");
        assert!(!save.requires_approval() && save.cost() == Cost::Standard);
        let wire = serde_json::to_value(&own).unwrap();
        assert_eq!(
            wire["reversibility"],
            serde_json::json!({"kind":"irreversible"})
        );
        assert_eq!(wire["audience"], "merchant");
    }
}
