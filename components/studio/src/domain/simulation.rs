//! Twin simulation of an admitted proposal: field-level diff of the replayed rows, projected revenue, margin and stock over a stated horizon with its assumptions, rule findings, and an explicit verdict for proposal parts the twin cannot simulate.
use super::proposal::{Change, Proposal};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use veyndo_core::{Result, bad};
use veyndo_intelligence::domain::price_guardrails::{assess, tax_bps, violations};
use veyndo_pricing::catalog::Product;
use veyndo_shop::Settings;

/// The simulation document version stored with a proposal.
pub const VERSION: u32 = 1;
/// The trailing window of recorded orders the demand baseline uses, in days.
pub const WINDOW_DAYS: i32 = 30;
/// The projection horizon, in days.
pub const HORIZON_DAYS: i64 = 30;

/// The stored result of replaying a proposal in the twin.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Simulation {
    pub version: u32,
    /// `simulated`, `partial` (some parts unsupported), `unsupported`, `notApplicable` or `failed`.
    pub status: String,
    /// How the twin replayed the proposal.
    pub twin: String,
    pub simulated_at: String,
    /// The settings revision and product clocks the replay ran against.
    pub basis: Basis,
    /// Every commerce-changing part of the proposal and whether the twin simulated it.
    pub parts: Vec<Part>,
    pub diff: Vec<FieldDiff>,
    pub impact: Option<Impact>,
    pub rules: Vec<RuleFinding>,
    /// Whether a blocking rule was violated; such a proposal cannot be applied.
    pub blocking: bool,
    /// Rule families the twin did not evaluate for this proposal.
    pub not_checked: Vec<String>,
    /// Why the replay failed, as a stable code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// The state a simulation was computed against.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Basis {
    /// The commerce settings revision; absent when the proposal reads no settings.
    pub policy_revision: Option<i64>,
    pub products: Vec<ProductBasis>,
}

/// One product's clocks before the replay.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductBasis {
    pub product_id: String,
    pub revision: i64,
    pub content_revision: i64,
}

/// One part of a proposal and the twin's verdict on it.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    /// `productChanges`, `experience`, `appAction`, `translation` or `operations`.
    pub kind: String,
    /// `simulated`, `unsupported` or `notApplicable`.
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// One changed field of a replayed record.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldDiff {
    pub entity: String,
    pub id: String,
    pub field: String,
    pub before: Value,
    pub after: Value,
}

/// The projected impact over the horizon, with the assumptions it rests on.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Impact {
    pub horizon_days: i64,
    pub window_days: i32,
    /// Stable assumption codes; the interface explains each.
    pub assumptions: Vec<String>,
    pub products: Vec<ProductImpact>,
    /// Sums per currency over the products with recorded demand.
    pub totals: Vec<CurrencyTotal>,
}

/// One product's projection without and with the proposal.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductImpact {
    pub product_id: String,
    pub currency: String,
    pub scale: u32,
    /// `recorded`; `noRecordedSales` when the window has no sales, or `notPermitted` when the actor may not read order
    /// figures: revenue is then not projected.
    pub demand: String,
    pub window_units: i64,
    pub window_orders: i64,
    pub baseline: Projection,
    pub proposed: Projection,
    /// Product-specific caveats as stable codes.
    pub notes: Vec<String>,
}

/// Units, money in minor units and stock at the end of the horizon; money is absent when it cannot be projected.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Projection {
    pub units: i64,
    pub gross_revenue_minor: Option<i64>,
    pub net_revenue_minor: Option<i64>,
    pub margin_minor: Option<i64>,
    pub ending_stock: i64,
    /// The day within the horizon on which stock runs out, if it does.
    pub stockout_day: Option<i64>,
}

/// Baseline and proposed sums of one currency.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrencyTotal {
    pub currency: String,
    pub scale: u32,
    pub baseline_gross_minor: i64,
    pub proposed_gross_minor: i64,
    pub delta_gross_minor: i64,
    /// Margin sums, only when every summed product has a known unit cost.
    pub baseline_margin_minor: Option<i64>,
    pub proposed_margin_minor: Option<i64>,
    pub delta_margin_minor: Option<i64>,
}

/// One rule the twin evaluated for one record.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleFinding {
    /// `priceGuardrail`, `priceIndication` or `revisionCurrent`.
    pub rule: String,
    pub product_id: String,
    /// `violated`, `passed` or `notConfigured`.
    pub outcome: String,
    /// `blocking` refuses the apply; `warning` informs the decision.
    pub severity: String,
    pub codes: Vec<String>,
}

/// Recorded demand of one product in the window.
pub struct Demand {
    pub product_id: String,
    pub units: i64,
    pub orders: i64,
}

/// Rule families never evaluated for product price/stock changes; stated so the review cannot read silence as a pass.
pub const NOT_CHECKED: &[&str] = &[
    "ruleBuilderConditions",
    "promotions",
    "customerGroupPrices",
    "shippingRules",
    "paymentRules",
    "destinationTaxes",
];

/// The twin's verdict on each commerce-changing part of a proposal.
pub fn parts(p: &Proposal) -> Vec<Part> {
    let part = |kind: &str, status: &str, reason: &str| Part {
        kind: kind.into(),
        status: status.into(),
        reason: (!reason.is_empty()).then(|| reason.into()),
    };
    let mut parts = vec![];
    if !p.changes.is_empty() {
        parts.push(part("productChanges", "simulated", ""));
    }
    if p.experience.is_some() {
        parts.push(part("experience", "unsupported", "noConversionModel"));
    }
    if p.app_action.is_some() {
        parts.push(part("appAction", "unsupported", "appOwnedState"));
    }
    if p.translation.is_some() {
        parts.push(part(
            "translation",
            "notApplicable",
            "draftsReviewedSeparately",
        ));
    }
    if !p.operations.is_empty() {
        parts.push(part("operations", "unsupported", "nativeWorkNotReplayed"));
    }
    parts
}

/// The overall status from the part verdicts.
pub fn status(parts: &[Part]) -> &'static str {
    let simulated = parts.iter().any(|p| p.status == "simulated");
    let unsupported = parts.iter().any(|p| p.status == "unsupported");
    match (simulated, unsupported) {
        (true, false) => "simulated",
        (true, true) => "partial",
        (false, true) => "unsupported",
        (false, false) => "notApplicable",
    }
}

/// The commerce fields of a product row that a replay can change, as JSON.
fn fields(p: &Product) -> [(&'static str, Value); 5] {
    [
        ("price", json!(p.price)),
        ("stock", json!(p.stock)),
        ("listPrice", json!(p.list_price)),
        ("regulationPrice", json!(p.regulation_price)),
        ("taxRate", json!(p.tax_rate)),
    ]
}

/// The changed fields between the rows before and after the replay.
pub fn diff(before: &[Product], after: &[Product]) -> Vec<FieldDiff> {
    let mut found = vec![];
    for b in before {
        let Some(a) = after.iter().find(|a| a.id == b.id) else {
            continue;
        };
        for ((field, old), (_, new)) in fields(b).into_iter().zip(fields(a)) {
            if old != new {
                found.push(FieldDiff {
                    entity: "product".into(),
                    id: b.id.clone(),
                    field: field.into(),
                    before: old,
                    after: new,
                });
            }
        }
    }
    found
}

/// Units sold, ending stock and stockout day when `demand` units are wanted over the horizon and `stock` is available.
pub fn sell(demand: i64, stock: i64) -> (i64, i64, Option<i64>) {
    let stock = stock.max(0);
    let sold = demand.min(stock);
    let stockout = (demand > 0 && stock < demand).then(|| stock * HORIZON_DAYS / demand);
    (sold, stock - sold, stockout)
}

/// One projection of `units` wanted at a gross unit price, with the product's tax and an optional net unit cost.
fn project(
    demand: Option<i64>,
    stock: i64,
    gross_minor: u64,
    tax: i64,
    cost: Option<u64>,
) -> Projection {
    let Some(wanted) = demand else {
        return Projection {
            units: 0,
            gross_revenue_minor: None,
            net_revenue_minor: None,
            margin_minor: None,
            ending_stock: stock.max(0),
            stockout_day: None,
        };
    };
    let (sold, ending, stockout) = sell(wanted, stock);
    let gross = i128::from(sold) * i128::from(gross_minor);
    let net = gross * 10000 / (10000 + i128::from(tax));
    let margin = cost.map(|c| net - i128::from(sold) * i128::from(c));
    let narrow = |v: i128| i64::try_from(v).ok();
    Projection {
        units: sold,
        gross_revenue_minor: narrow(gross),
        net_revenue_minor: narrow(net),
        margin_minor: margin.and_then(narrow),
        ending_stock: ending,
        stockout_day: stockout,
    }
}

/// Demand over the horizon from the window's recorded units, rounded down.
fn horizon_units(window_units: i64) -> i64 {
    window_units * HORIZON_DAYS / i64::from(WINDOW_DAYS)
}

/// Diff, projection and rule findings of the product changes, from the rows before and after the twin replay.
pub fn changes(
    s: &Settings,
    changes: &[Change],
    before: &[Product],
    after: &[Product],
    demand: Option<&[Demand]>,
) -> Result<(Vec<FieldDiff>, Impact, Vec<RuleFinding>)> {
    let mut products = vec![];
    let mut rules = vec![];
    let mut assumptions = vec![
        "demandFromRecordedOrders",
        "priceElasticityNotModelled",
        "stockLimitsSales",
        "noReplenishment",
        "baseGrossPriceOnly",
    ];
    for c in changes {
        let (Some(b), Some(a)) = (
            before.iter().find(|p| p.id == c.product_id),
            after.iter().find(|p| p.id == c.product_id),
        ) else {
            continue;
        };
        // The current row prices the baseline; the replayed row prices the proposal.
        let base = assess(s, b, &priced(c, b))?.ok_or(bad("Unpriced product"))?;
        let next = assess(s, b, &priced(c, a))?.ok_or(bad("Unpriced product"))?;
        let tax = tax_bps(b)?;
        let cost = base.corridor.and_then(|r| r.unit_cost_net_minor);
        let recorded = demand
            .into_iter()
            .flatten()
            .find(|d| d.product_id == c.product_id);
        let wanted = recorded.map(|d| horizon_units(d.units));
        let mut notes = vec![];
        if cost.is_none() {
            notes.push("unitCostUnknown".to_string());
        }
        if !b.advanced_prices.is_empty() {
            notes.push("advancedPricesIgnored".to_string());
        }
        products.push(ProductImpact {
            product_id: c.product_id.clone(),
            currency: base.currency.clone(),
            scale: base.scale,
            demand: match (demand, recorded) {
                (None, _) => "notPermitted",
                (Some(_), Some(_)) => "recorded",
                (Some(_), None) => "noRecordedSales",
            }
            .into(),
            window_units: recorded.map_or(0, |d| d.units),
            window_orders: recorded.map_or(0, |d| d.orders),
            baseline: project(wanted, i64::from(b.stock), base.proposed, tax, cost),
            proposed: project(wanted, i64::from(a.stock), next.proposed, tax, cost),
            notes,
        });
        if c.price.is_some() {
            let codes = violations(s, b, c)?;
            rules.push(RuleFinding {
                rule: "priceGuardrail".into(),
                product_id: c.product_id.clone(),
                outcome: if base.corridor.is_none() {
                    "notConfigured"
                } else if codes.is_empty() {
                    "passed"
                } else {
                    "violated"
                }
                .into(),
                severity: "blocking".into(),
                codes: codes.into_iter().map(String::from).collect(),
            });
            let indication = indication(b, a);
            rules.push(RuleFinding {
                rule: "priceIndication".into(),
                product_id: c.product_id.clone(),
                outcome: if indication.is_empty() {
                    "passed"
                } else {
                    "violated"
                }
                .into(),
                severity: "warning".into(),
                codes: indication.into_iter().map(String::from).collect(),
            });
        }
    }
    if products
        .iter()
        .any(|p| p.notes.iter().any(|n| n == "unitCostUnknown"))
    {
        assumptions.push("marginNeedsUnitCost");
    }
    let impact = Impact {
        horizon_days: HORIZON_DAYS,
        window_days: WINDOW_DAYS,
        assumptions: assumptions.into_iter().map(String::from).collect(),
        totals: totals(&products),
        products,
    };
    Ok((diff(before, after), impact, rules))
}

/// The change as a price-only change at a row's current price, to measure that row with the guardrail arithmetic.
fn priced(c: &Change, row: &Product) -> Change {
    Change {
        price: Some(row.price),
        stock: None,
        ..c.clone()
    }
}

/// Price indication findings of the replayed row: a list price that no longer shows a reduction, or a shown reduction without the lowest prior price.
pub fn indication(before: &Product, after: &Product) -> Vec<&'static str> {
    let mut codes = vec![];
    let list = after.list_price.filter(|l| !l.is_zero());
    if list.is_some_and(|l| l <= after.price) {
        codes.push("listPriceNotAbovePrice");
    }
    if list.is_some_and(|l| l > after.price)
        && after.price < before.price
        && after.regulation_price.is_none_or(|r| r.is_zero())
    {
        codes.push("lowestPriorPriceMissing");
    }
    codes
}

/// Baseline and proposed sums per currency over products with recorded demand.
fn totals(products: &[ProductImpact]) -> Vec<CurrencyTotal> {
    let mut totals: Vec<CurrencyTotal> = vec![];
    for p in products.iter().filter(|p| p.demand == "recorded") {
        let (Some(bg), Some(pg)) = (
            p.baseline.gross_revenue_minor,
            p.proposed.gross_revenue_minor,
        ) else {
            continue;
        };
        let index = match totals.iter().position(|t| t.currency == p.currency) {
            Some(i) => i,
            None => {
                totals.push(CurrencyTotal {
                    currency: p.currency.clone(),
                    scale: p.scale,
                    baseline_gross_minor: 0,
                    proposed_gross_minor: 0,
                    delta_gross_minor: 0,
                    baseline_margin_minor: Some(0),
                    proposed_margin_minor: Some(0),
                    delta_margin_minor: Some(0),
                });
                totals.len() - 1
            }
        };
        let t = &mut totals[index];
        t.baseline_gross_minor = t.baseline_gross_minor.saturating_add(bg);
        t.proposed_gross_minor = t.proposed_gross_minor.saturating_add(pg);
        t.delta_gross_minor = t
            .proposed_gross_minor
            .saturating_sub(t.baseline_gross_minor);
        let add = |sum: Option<i64>, v: Option<i64>| Some(sum?.saturating_add(v?));
        t.baseline_margin_minor = add(t.baseline_margin_minor, p.baseline.margin_minor);
        t.proposed_margin_minor = add(t.proposed_margin_minor, p.proposed.margin_minor);
        t.delta_margin_minor = t
            .proposed_margin_minor
            .zip(t.baseline_margin_minor)
            .map(|(p, b)| p.saturating_sub(b));
    }
    totals
}

/// Whether any blocking rule was violated.
pub fn blocking(rules: &[RuleFinding]) -> bool {
    rules
        .iter()
        .any(|r| r.severity == "blocking" && r.outcome == "violated")
}

/// The simulation of a proposal the twin could not replay, with the reason code.
pub fn failed(p: &Proposal, basis: Basis, at: String, reason: &str) -> Simulation {
    Simulation {
        version: VERSION,
        status: "failed".into(),
        twin: TWIN.into(),
        simulated_at: at,
        basis,
        parts: parts(p),
        diff: vec![],
        impact: None,
        rules: vec![],
        blocking: false,
        not_checked: NOT_CHECKED.iter().map(|s| s.to_string()).collect(),
        reason: Some(reason.into()),
    }
}

/// The simulation of a proposal without product changes: nothing to replay, only the verdict per part.
pub fn unreplayed(p: &Proposal, policy_revision: Option<i64>, at: String) -> Simulation {
    let parts = parts(p);
    Simulation {
        version: VERSION,
        status: status(&parts).into(),
        twin: TWIN.into(),
        simulated_at: at,
        basis: Basis {
            policy_revision,
            products: vec![],
        },
        parts,
        diff: vec![],
        impact: None,
        rules: vec![],
        blocking: false,
        not_checked: NOT_CHECKED.iter().map(|s| s.to_string()).collect(),
        reason: None,
    }
}

/// The twin a replay runs in: the targeted shop's current rows inside a transaction that is always rolled back.
pub const TWIN: &str = "transactionalReplay";

/// Whether a saved simulation lets its proposal be applied: a failed replay or a blocking violation refuses it.
pub fn admits(simulation: &Value) -> std::result::Result<(), &'static str> {
    if simulation.is_null() {
        return Ok(());
    }
    let s: SavedVerdict = veyndo_core::document::view(simulation);
    if s.status.as_deref() == Some("failed") {
        return Err("The shop twin could not simulate this proposal; ask the agent again");
    }
    if s.blocking == Some(true) {
        return Err("The shop twin found blocking rule violations in this proposal");
    }
    Ok(())
}

/// The fields of a saved simulation the apply decision reads.
#[derive(Deserialize, Default)]
#[serde(default)]
struct SavedVerdict {
    #[serde(deserialize_with = "veyndo_core::document::lenient")]
    status: Option<String>,
    #[serde(deserialize_with = "veyndo_core::document::lenient")]
    blocking: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use veyndo_pricing::decimal::Decimal;

    fn product(id: &str, price: f64, stock: i32, list: Option<f64>) -> Product {
        serde_json::from_value(json!({
            "extra":{},"id":id,"name":"Lamp","category":"","description":"","price":price,
            "tax_rate":19.0,"stock":stock,"revision":3,"list_price":list,"regulation_price":null,
            "reference_price":null,"advanced_prices":[],"min_purchase":1,"purchase_steps":1,
            "max_purchase":null,"parent_id":null,"options":{},"media":[],"properties":{},
            "delivery_days":1
        }))
        .unwrap()
    }

    fn settings(corridor: Option<Value>) -> Settings {
        let mut s: Settings =
            serde_json::from_str(include_str!("../../../../fixtures/demo-settings.json")).unwrap();
        let code = s.currencies.pricing_currency.clone();
        let mut policy = json!({"corridors":[],"autonomy":{"enabled":false,"maxChangeBps":500,"productsPerDay":5}});
        if let Some(mut c) = corridor {
            c["currency"] = json!(code);
            policy["corridors"] = json!([c]);
        }
        s.ai_policy = serde_json::from_value(policy).unwrap();
        s
    }

    fn change(price: Option<f64>, stock: Option<i32>) -> Change {
        Change {
            product_id: "lamp".into(),
            expected_revision: 3,
            expected_content_revision: None,
            price: price.map(|p| Decimal::from_wire(p).unwrap()),
            stock,
        }
    }

    fn proposal(changes: Vec<Change>, experience: Option<Value>) -> Proposal {
        serde_json::from_value(json!({"summary":"s","changes":changes,"experience":experience}))
            .unwrap()
    }

    #[test]
    fn a_price_change_projects_revenue_margin_and_stock_from_recorded_demand() {
        let s = settings(Some(
            json!({"productId":"lamp","minimumMinor":5000,"maximumMinor":20000,"unitCostNetMinor":4000}),
        ));
        let before = [product("lamp", 119., 20, None)];
        let after = [product("lamp", 107.1, 20, None)];
        let demand = [Demand {
            product_id: "lamp".into(),
            units: 12,
            orders: 9,
        }];
        let (diff, impact, rules) = changes(
            &s,
            &[change(Some(107.1), None)],
            &before,
            &after,
            Some(&demand),
        )
        .unwrap();
        assert_eq!(diff.len(), 1);
        assert_eq!(
            (diff[0].field.as_str(), &diff[0].before, &diff[0].after),
            ("price", &json!(119.0), &json!(107.1))
        );
        let p = &impact.products[0];
        assert_eq!(p.demand, "recorded");
        // 12 units at 119.00 gross: 1428.00 gross, 1200.00 net at 19 %, margin 1200.00 - 12 * 40.00.
        assert_eq!(p.baseline.gross_revenue_minor, Some(142800));
        assert_eq!(p.baseline.net_revenue_minor, Some(120000));
        assert_eq!(p.baseline.margin_minor, Some(72000));
        assert_eq!(p.proposed.gross_revenue_minor, Some(128520));
        assert_eq!(p.proposed.net_revenue_minor, Some(108000));
        assert_eq!(p.proposed.margin_minor, Some(60000));
        assert_eq!(
            (p.proposed.ending_stock, p.proposed.stockout_day),
            (8, None)
        );
        assert_eq!(impact.totals[0].delta_gross_minor, -14280);
        assert_eq!(impact.totals[0].delta_margin_minor, Some(-12000));
        assert!(
            impact
                .assumptions
                .iter()
                .any(|a| a == "priceElasticityNotModelled")
        );
        assert!(
            !impact
                .assumptions
                .iter()
                .any(|a| a == "marginNeedsUnitCost")
        );
        assert_eq!(rules[0].outcome, "passed");
        assert!(!blocking(&rules));
    }

    #[test]
    fn a_guardrail_violation_blocks_and_names_its_rule() {
        let s = settings(Some(
            json!({"productId":"lamp","minimumMinor":10000,"maximumMinor":20000}),
        ));
        let before = [product("lamp", 119., 20, None)];
        let after = [product("lamp", 90., 20, None)];
        let (_, _, rules) =
            changes(&s, &[change(Some(90.), None)], &before, &after, Some(&[])).unwrap();
        assert_eq!(rules[0].rule, "priceGuardrail");
        assert_eq!(rules[0].outcome, "violated");
        assert_eq!(rules[0].codes, ["corridorMinimum"]);
        assert!(blocking(&rules));
    }

    #[test]
    fn missing_sales_and_unit_cost_are_stated_not_estimated() {
        let s = settings(None);
        let before = [product("lamp", 119., 20, None)];
        let after = [product("lamp", 129., 20, None)];
        let (_, impact, rules) =
            changes(&s, &[change(Some(129.), None)], &before, &after, Some(&[])).unwrap();
        let p = &impact.products[0];
        assert_eq!(p.demand, "noRecordedSales");
        assert_eq!(p.proposed.gross_revenue_minor, None);
        assert!(p.notes.iter().any(|n| n == "unitCostUnknown"));
        assert!(impact.totals.is_empty());
        assert!(
            impact
                .assumptions
                .iter()
                .any(|a| a == "marginNeedsUnitCost")
        );
        assert_eq!(rules[0].outcome, "notConfigured");
        let (_, hidden, _) =
            changes(&s, &[change(Some(129.), None)], &before, &after, None).unwrap();
        assert_eq!(hidden.products[0].demand, "notPermitted");
        assert_eq!(hidden.products[0].window_units, 0);
    }

    #[test]
    fn stock_limits_projected_sales_and_reports_the_stockout_day() {
        assert_eq!(sell(30, 10), (10, 0, Some(10)));
        assert_eq!(sell(10, 30), (10, 20, None));
        assert_eq!(sell(0, 5), (0, 5, None));
        assert_eq!(sell(4, -2), (0, 0, Some(0)));
        let s = settings(None);
        let before = [product("lamp", 10., 50, None)];
        let after = [product("lamp", 10., 5, None)];
        let demand = [Demand {
            product_id: "lamp".into(),
            units: 20,
            orders: 20,
        }];
        let (diff, impact, rules) =
            changes(&s, &[change(None, Some(5))], &before, &after, Some(&demand)).unwrap();
        assert_eq!(diff[0].field, "stock");
        assert_eq!(impact.products[0].proposed.units, 5);
        assert_eq!(impact.products[0].proposed.stockout_day, Some(7));
        assert_eq!(impact.totals[0].delta_gross_minor, -15000);
        assert!(rules.is_empty(), "a stock-only change has no price rules");
    }

    #[test]
    fn price_indication_warns_about_list_and_lowest_prior_prices() {
        let shown = product("lamp", 100., 1, Some(120.));
        assert!(
            indication(&shown, &product("lamp", 90., 1, Some(120.)))
                .contains(&"lowestPriorPriceMissing")
        );
        assert_eq!(
            indication(&shown, &product("lamp", 125., 1, Some(120.))),
            ["listPriceNotAbovePrice"]
        );
        assert!(indication(&shown, &product("lamp", 110., 1, None)).is_empty());
        let mut with_lowest = product("lamp", 90., 1, Some(120.));
        with_lowest.regulation_price = Some(Decimal::from_wire(95.).unwrap());
        assert!(indication(&shown, &with_lowest).is_empty());
    }

    #[test]
    fn parts_the_twin_cannot_replay_are_named_explicitly() {
        let p = proposal(
            vec![change(Some(1.), None)],
            Some(json!({"mode":"discovery","headline":"x"})),
        );
        let found = parts(&p);
        assert_eq!(found[0].status, "simulated");
        assert_eq!(
            (found[1].kind.as_str(), found[1].status.as_str()),
            ("experience", "unsupported")
        );
        assert_eq!(status(&found), "partial");
        assert_eq!(
            status(&parts(&proposal(
                vec![],
                Some(json!({"mode":"discovery","headline":"x"}))
            ))),
            "unsupported"
        );
        assert_eq!(status(&parts(&proposal(vec![], None))), "notApplicable");
    }

    #[test]
    fn a_failed_or_blocking_simulation_refuses_apply_and_legacy_proposals_pass() {
        assert!(admits(&Value::Null).is_ok());
        assert!(admits(&json!({"status":"simulated","blocking":false})).is_ok());
        assert!(admits(&json!({"status":"partial","blocking":true})).is_err());
        assert!(admits(&json!({"status":"failed"})).is_err());
    }
}
