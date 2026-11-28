//! One immutable native operation record for dispatch, schemas, typed planner facts, MCP discovery and bounded agent reads.
use super::{admission, catalog, dispatch, facts, schema};
use crate::apps;
use crate::foundation::App;
use dispatch::Owner;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;
use std::sync::LazyLock;
use veyndo_core::Error;
use veyndo_core::Result;
use veyndo_core::operation_facts::{Cost, OperationFacts};
use veyndo_core::request_context::RequestContext;
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Effect {
    Read,
    Write,
}
pub(crate) struct Operation {
    pub(crate) name: &'static str,
    description: &'static str,
    pub(super) owner: Owner,
    input: Value,
    output: Option<Value>,
    pub(super) effect: Effect,
    agent_read: bool,
    pub(super) facts: OperationFacts,
    pub(super) preconditions: Preconditions,
}
/// Call preconditions read from the published input contract, so they cannot drift from it.
#[derive(Clone, Copy, Serialize, Default, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub(super) struct Preconditions {
    /// The caller must pass the record revision it read (optimistic concurrency).
    expected_revision: bool,
    /// The caller must pass an idempotency or request key.
    idempotency_key: bool,
    /// The caller must state an explicit approval in the call itself.
    explicit_approval: bool,
}
impl Preconditions {
    fn of(input: &Value) -> Self {
        let mut found = Self::default();
        found.collect(input, 0);
        found
    }
    /// Follows required properties only: an optional nested field is no precondition.
    fn collect(&mut self, schema: &Value, depth: usize) {
        let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
            return;
        };
        let required = schema.get("required").and_then(Value::as_array);
        for (name, property) in properties {
            if !required.is_some_and(|names| names.iter().any(|n| n == name)) {
                continue;
            }
            match name.as_str() {
                "revision" | "settingsRevision" => self.expected_revision = true,
                "idempotency_key" | "requestKey" => self.idempotency_key = true,
                "approve" => self.explicit_approval = true,
                _ if depth < 4 => self.collect(property, depth + 1),
                _ => {}
            }
        }
    }
}
pub(crate) static OPERATIONS: LazyLock<Vec<Operation>> = LazyLock::new(|| {
    assemble(catalog::definitions(), facts::FACTS).unwrap_or_else(|error| panic!("{error}"))
});
/// Joins every catalogue entry with its typed facts; a missing, stale or contradictory declaration is refused.
fn assemble(
    definitions: Vec<(&'static str, &'static str)>,
    declared: &[(&'static str, OperationFacts)],
) -> std::result::Result<Vec<Operation>, String> {
    let mut facts = std::collections::HashMap::new();
    for (name, value) in declared {
        if facts.insert(*name, *value).is_some() {
            return Err(format!("{name}: facts are declared twice"));
        }
    }
    let mut seen = std::collections::HashSet::new();
    let operations = definitions
        .into_iter()
        .map(|(name, description)| {
            if !seen.insert(name) {
                return Err(format!("Duplicate native operation {name}"));
            }
            let facts = facts.remove(name).ok_or_else(|| {
                format!("{name}: a native operation needs typed facts in capabilities/facts.rs")
            })?;
            facts.check(name)?;
            let agent_read = matches!(
                name,
                "knowledge.behavior"
                    | "merchant.reports.read"
                    | "studio.workspace.work.read"
                    | "merchant.apps.search"
                    | "merchant.apps.read"
                    | "catalog.search"
                    | "catalog.detail"
                    | "merchant.catalog.context"
                    | "merchant.products"
                    | "merchant.product.content"
                    | "merchant.orders"
                    | "merchant.order"
                    | "merchant.commerce.read"
                    | "merchant.company"
                    | "automation.list"
                    | "automation.catalog"
                    | "automation.compare_history"
                    | "knowledge.claims"
                    | "knowledge.experiments"
                    | "knowledge.experiment.report"
                    | "knowledge.experiment.suggest"
                    | "knowledge.search"
                    | "knowledge.external"
                    | "knowledge.workspace"
                    | "knowledge.product"
                    | "knowledge.source.detail"
                    | "merchant.operations.list"
                    | "merchant.operations.check"
            );
            // Unclassified operations deliberately retain conservative mutating hints.
            let read = agent_read
                || name == "automation.validate_flow"
                || name == "merchant.import.inspect"
                || name == "merchant.run.status"
                || name == "merchant.run.explain"
                || name == "merchant.scenario.status"
                || name == "merchant.scenario.list"
                || name == "merchant.scenario.readiness"
                || name == "merchant.scenario.attacks"
                || name == "knowledge.autonomy.status"
                || name == "merchant.agents.status"
                || [
                    "merchant.payment.details",
                    "merchant.payment.instructions",
                    "checkout.payment.instructions",
                    "checkout.payment.methods",
                ]
                .contains(&name)
                || matches!(
                    name,
                    "catalog.facts"
                        | "checkout.options"
                        | "cart.quote"
                        | "cart.offer"
                        | "offer.verify"
                );
            let effect = if read { Effect::Read } else { Effect::Write };
            if effect == Effect::Read && facts.writes {
                return Err(format!(
                    "{name}: a read-only operation cannot declare writes"
                ));
            }
            let input = schema::input(name);
            Ok(Operation {
                name,
                description,
                owner: Owner::for_name(name),
                preconditions: Preconditions::of(&input),
                input,
                output: schema::output(name),
                effect,
                agent_read,
                facts,
            })
        })
        .collect::<std::result::Result<Vec<_>, String>>()?;
    if let Some(name) = facts.keys().next() {
        return Err(format!("{name}: facts name no native operation"));
    }
    for operation in &operations {
        let target = |name: &str| operations.iter().find(|other| other.name == name);
        if let Some(by) = operation.facts.compensation()
            && !target(by).is_some_and(|other| other.facts.writes)
        {
            return Err(format!(
                "{}: compensation {by} is no writing operation",
                operation.name
            ));
        }
        if let Some(missing) = operation
            .facts
            .requires
            .iter()
            .find(|name| target(name).is_none())
        {
            return Err(format!(
                "{}: required operation {missing} does not exist",
                operation.name
            ));
        }
    }
    Ok(operations)
}
pub(super) fn find(name: &str) -> Option<&'static Operation> {
    OPERATIONS.iter().find(|operation| operation.name == name)
}
pub(crate) fn tool_schema(name: &str) -> Value {
    find(name)
        .map(|operation| operation.input.clone())
        .unwrap_or_else(|| json!({}))
}
pub(crate) fn agent_reads(a: &App, h: &RequestContext) -> Vec<Value> {
    OPERATIONS
        .iter()
        .filter(|operation| {
            operation.agent_read && operation.effect == Effect::Read && operation.admitted(a, h)
        })
        .map(|operation| operation.tool())
        .collect()
}
impl Operation {
    pub(crate) fn input_schema(&self) -> Value {
        self.input.clone()
    }
    pub(super) fn check_output(&self, value: Value) -> Result<Value> {
        if let Some(schema) = &self.output {
            apps::validate_native_input(schema, &value).map_err(|_| Error::output_contract())?;
        }
        Ok(value)
    }
    pub(crate) fn admitted(&self, _a: &App, h: &RequestContext) -> bool {
        self.admitted_now(h)
    }
    /// Current admission from the request principal alone; agent credentials stay limited to reads.
    pub(super) fn admitted_now(&self, h: &RequestContext) -> bool {
        if h.principal.agent_key.is_some()
            && self.effect != Effect::Read
            && !self.name.starts_with("merchant.run.")
        {
            return false;
        }
        admission::visible(h, self.name)
    }
    /// The planner view of this operation: its contract summary, facts and call preconditions.
    pub(super) fn plan(&self) -> OperationPlan<'_> {
        OperationPlan {
            name: self.name,
            description: self.description,
            read_only: self.effect == Effect::Read,
            cost: self.facts.cost(),
            requires_approval: self.facts.requires_approval(),
            facts: &self.facts,
            preconditions: self.preconditions,
        }
    }
    pub(crate) fn description(&self) -> Value {
        json!({"name":self.name,"description":self.description})
    }
    /// The MCP tool: `readOnly` keeps the admission effect (what agent credentials may run), while the
    /// annotations follow the typed facts.
    pub(crate) fn tool(&self) -> Value {
        let mut tool = super::ToolContract::describe(
            self.name,
            self.description,
            &self.input,
            self.output.as_ref(),
            self.effect == Effect::Read,
        );
        super::ToolContract::with_hints(&mut tool, self.facts.hints());
        tool
    }
}
/// The typed planner view of one operation, shared by the planner read and the offline export.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct OperationPlan<'a> {
    pub(super) name: &'a str,
    description: &'a str,
    read_only: bool,
    cost: Cost,
    requires_approval: bool,
    facts: &'a OperationFacts,
    preconditions: Preconditions,
}
/// One operation in the offline contract export.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportedOperation<'a> {
    name: &'a str,
    description: &'a str,
    input_schema: &'a Value,
    read_only: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_schema: Option<&'a Value>,
    cost: Cost,
    requires_approval: bool,
    facts: &'a OperationFacts,
    preconditions: Preconditions,
}
/// Build-time contract export is offline and includes no actor credentials or records.
pub(crate) fn export() -> Value {
    let operations = OPERATIONS
        .iter()
        .map(|operation| ExportedOperation {
            name: operation.name,
            description: operation.description,
            input_schema: &operation.input,
            read_only: operation.effect == Effect::Read,
            output_schema: operation.output.as_ref(),
            cost: operation.facts.cost(),
            requires_approval: operation.facts.requires_approval(),
            facts: &operation.facts,
            preconditions: operation.preconditions,
        })
        .collect::<Vec<_>>();
    json!({"version":1,"operations":operations})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marketing;
    use veyndo_catalog::types::catalog_page::CatalogCriteria;
    #[test]
    fn export_matches_the_indexed_shape() {
        let indexed = OPERATIONS
            .iter()
            .map(|operation| {
                let mut value = json!({"name":operation.name,"description":operation.description,
                "inputSchema":operation.input,"readOnly":operation.effect==Effect::Read});
                if let Some(schema) = &operation.output {
                    value["outputSchema"] = schema.clone();
                }
                let plan = serde_json::to_value(operation.plan()).unwrap();
                for key in ["cost", "requiresApproval", "facts", "preconditions"] {
                    value[key] = plan[key].clone();
                }
                value
            })
            .collect::<Vec<_>>();
        assert!(
            OPERATIONS
                .iter()
                .any(|operation| operation.output.is_some())
        );
        assert_eq!(export(), json!({"version":1,"operations":indexed}));
    }
    #[test]
    fn grid_properties_match_the_indexed_shape() {
        let mut products = crate::operations::grid_fields("products").unwrap();
        products["preparation"] = json!({"type":"boolean"});
        products["after"] = json!({"type":"string"});
        products["limit"] = json!({"type":"integer","minimum":1,"maximum":100});
        assert_eq!(
            find("merchant.products").unwrap().input["properties"],
            products
        );
        for (name, grid) in [
            ("merchant.orders", "orders"),
            ("merchant.customers", "customers"),
        ] {
            let mut p = crate::operations::grid_fields(grid).unwrap();
            p["after"] = json!({"type":"string"});
            p["limit"] = json!({"type":"integer","minimum":1,"maximum":100});
            assert_eq!(find(name).unwrap().input["properties"], p);
        }
    }
    #[test]
    fn catalog_contract_preserves_native_categories_and_search_alias() {
        let schema = tool_schema("catalog.search");
        for value in [
            json!({"categoryId":"cycle","search":"lamp"}),
            json!({"query":"lamp"}),
        ] {
            apps::validate_native_input(&schema, &value).unwrap();
            let parsed: CatalogCriteria = serde_json::from_value(value).unwrap();
            assert_eq!(parsed.search.as_deref(), Some("lamp"));
            parsed.page_size().unwrap();
        }
        assert!(apps::validate_native_input(&schema, &json!({"categoryId":true})).is_err());
    }
    #[test]
    fn flow_check_result_cannot_claim_a_save_or_omit_its_revision() {
        let operation = find(marketing::DRAFT_CAPABILITY.0).unwrap();
        assert!(
            operation
                .check_output(json!({"valid":true,"saved":false,"revision":0}))
                .is_ok()
        );
        for value in [
            json!({"valid":true,"saved":true,"revision":0}),
            json!({"valid":true,"saved":false}),
            json!({"valid":true,"saved":false,"revision":-1}),
        ] {
            assert!(operation.check_output(value).is_err());
        }
    }
    #[test]
    fn every_operation_has_one_contract_and_agent_reads_are_conservative() {
        assert!(OPERATIONS.len() > 100);
        for operation in OPERATIONS.iter() {
            assert_eq!(operation.input["type"], "object", "{}", operation.name);
            assert_eq!(find(operation.name).unwrap().name, operation.name);
            if operation.agent_read {
                assert!(operation.effect == Effect::Read);
            }
        }
        assert_eq!(
            find("merchant.payment.details").unwrap().tool()["annotations"]["readOnlyHint"],
            true
        );
        for name in [
            "merchant.apply",
            "checkout.complete",
            "merchant.product.save",
            "automation.save",
        ] {
            let operation = find(name).unwrap();
            assert!(!operation.agent_read);
            assert_eq!(operation.tool()["annotations"]["readOnlyHint"], false);
        }
    }

    #[test]
    fn mcp_hints_follow_the_typed_facts() {
        use veyndo_core::operation_facts::{Reversibility, Risk};
        for operation in OPERATIONS.iter() {
            let tool = operation.tool();
            let facts = &operation.facts;
            assert_eq!(
                tool["annotations"]["readOnlyHint"], !facts.writes,
                "{}",
                operation.name
            );
            assert_eq!(
                tool["annotations"]["destructiveHint"],
                facts.writes
                    && (facts.risk == Risk::High
                        || facts.reversibility == Reversibility::Irreversible),
                "{}",
                operation.name
            );
            // Admission is unchanged: `readOnly` still states what agent credentials may run.
            assert_eq!(
                tool["readOnly"],
                operation.effect == Effect::Read,
                "{}",
                operation.name
            );
        }
        let currencies = find("currency.list").unwrap().tool();
        assert_eq!(currencies["annotations"]["readOnlyHint"], true);
        assert_eq!(currencies["annotations"]["destructiveHint"], false);
        assert_eq!(currencies["annotations"]["openWorldHint"], false);
    }
    fn member(permissions: &[&str], personal: bool) -> RequestContext {
        let mut h = RequestContext::new();
        for (key, value) in [
            ("x-rac-role", "member".to_owned()),
            ("x-rac-tenant", "demo".to_owned()),
            ("x-tenant", "demo".to_owned()),
            ("x-rac-user", "u1".to_owned()),
            (
                "x-rac-permissions",
                serde_json::to_string(permissions).unwrap(),
            ),
        ] {
            h.insert(key, value.parse().unwrap());
        }
        h.principal.personal_session = personal;
        h
    }
    #[test]
    fn declared_permissions_match_admission() {
        use veyndo_core::operation_facts::{Audience, Condition};
        for operation in OPERATIONS.iter() {
            let facts = &operation.facts;
            let name = operation.name;
            let anonymous = admission::visible(&RequestContext::new(), name);
            assert_eq!(
                anonymous,
                facts.audience == Audience::Shopper,
                "{name}: audience differs from admission"
            );
            if anonymous {
                continue;
            }
            let personal = facts.conditions.contains(&Condition::PersonalSession);
            assert!(
                admission::visible(&member(facts.permissions, personal), name),
                "{name}: declared permissions are not sufficient"
            );
            if personal {
                assert!(
                    !admission::visible(&member(facts.permissions, false), name),
                    "{name}: a personal session is declared but not required"
                );
            }
            for missing in facts.permissions {
                let rest = facts
                    .permissions
                    .iter()
                    .copied()
                    .filter(|permission| permission != missing)
                    .collect::<Vec<_>>();
                assert!(
                    !admission::visible(&member(&rest, personal), name),
                    "{name}: {missing} is declared but not required"
                );
            }
        }
    }
    #[test]
    fn every_operation_is_declared_once_with_consistent_facts() {
        use std::collections::HashSet;
        let declared = facts::FACTS
            .iter()
            .map(|(name, _)| *name)
            .collect::<HashSet<_>>();
        let names = OPERATIONS.iter().map(|o| o.name).collect::<HashSet<_>>();
        assert_eq!(declared, names);
        for name in [
            "checkout.complete",
            "merchant.payment",
            "merchant.apply",
            "team.role.delete",
        ] {
            assert!(find(name).unwrap().facts.requires_approval(), "{name}");
        }
        for name in [
            "catalog.search",
            "merchant.products",
            "cart.replace",
            "merchant.plan",
        ] {
            assert!(!find(name).unwrap().facts.requires_approval(), "{name}");
        }
        assert!(
            find("merchant.category.save")
                .unwrap()
                .preconditions
                .expected_revision
        );
        assert!(
            !find("merchant.category.create")
                .unwrap()
                .preconditions
                .expected_revision
        );
        assert!(
            find("merchant.apply")
                .unwrap()
                .preconditions
                .explicit_approval
        );
        assert!(
            find("checkout.complete")
                .unwrap()
                .preconditions
                .idempotency_key
        );
    }
    #[test]
    fn assembly_refuses_missing_stale_and_contradictory_facts() {
        use veyndo_core::operation_facts::{Audience, OperationFacts, Reversibility, Risk};
        let read = OperationFacts::read(Audience::Shopper, &[]);
        let write =
            OperationFacts::write(Audience::Shopper, &[], Reversibility::Reversible, Risk::Low);
        let error = |definitions: Vec<(&'static str, &'static str)>,
                     declared: &[(&'static str, OperationFacts)]| {
            assemble(definitions, declared).err().unwrap()
        };
        let one = || vec![("cart.create", "Create cart")];
        assert!(error(one(), &[]).contains("needs typed facts"));
        assert!(
            error(one(), &[("cart.create", write), ("cart.gone", write)])
                .contains("name no native")
        );
        assert!(error(one(), &[("cart.create", write), ("cart.create", write)]).contains("twice"));
        assert!(
            error(
                vec![("cart.create", "a"), ("cart.create", "b")],
                &[("cart.create", write)]
            )
            .contains("Duplicate")
        );
        assert!(
            error(
                vec![("catalog.search", "Read")],
                &[("catalog.search", write)]
            )
            .contains("cannot declare writes")
        );
        let compensated = OperationFacts::write(
            Audience::Shopper,
            &[],
            Reversibility::Compensatable { by: "cart.missing" },
            Risk::Low,
        );
        assert!(error(one(), &[("cart.create", compensated)]).contains("no writing operation"));
        let by_read = OperationFacts::write(
            Audience::Shopper,
            &[],
            Reversibility::Compensatable {
                by: "catalog.search",
            },
            Risk::Low,
        );
        assert!(
            error(
                vec![("cart.create", "a"), ("catalog.search", "b")],
                &[("cart.create", by_read), ("catalog.search", read)]
            )
            .contains("no writing operation")
        );
        assert!(
            error(one(), &[("cart.create", write.requires(&["cart.missing"]))])
                .contains("does not exist")
        );
        let irreversible_low = OperationFacts::write(
            Audience::Shopper,
            &[],
            Reversibility::Irreversible,
            Risk::Low,
        );
        assert!(error(one(), &[("cart.create", irreversible_low)]).contains("at least medium"));
        assert_eq!(assemble(one(), &[("cart.create", write)]).unwrap().len(), 1);
    }
}
