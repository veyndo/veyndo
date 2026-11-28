/** Wire shapes of the stored twin simulation and agent explanation, and the pure decisions the review derives from them. */
export type SimulationPart = { kind: string; status: string; reason?: string };
export type FieldDiff = {
  entity: string;
  id: string;
  field: string;
  before: unknown;
  after: unknown;
};
export type Projection = {
  units: number;
  grossRevenueMinor: number | null;
  netRevenueMinor: number | null;
  marginMinor: number | null;
  endingStock: number;
  stockoutDay: number | null;
};
export type ProductImpact = {
  productId: string;
  currency: string;
  scale: number;
  demand: string;
  windowUnits: number;
  windowOrders: number;
  baseline: Projection;
  proposed: Projection;
  notes: string[];
};
export type CurrencyTotal = {
  currency: string;
  scale: number;
  baselineGrossMinor: number;
  proposedGrossMinor: number;
  deltaGrossMinor: number;
  baselineMarginMinor: number | null;
  proposedMarginMinor: number | null;
  deltaMarginMinor: number | null;
};
export type RuleFinding = {
  rule: string;
  productId: string;
  outcome: string;
  severity: string;
  codes: string[];
};
export type Simulation = {
  version: number;
  status: string;
  twin: string;
  simulatedAt: string;
  parts: SimulationPart[];
  diff: FieldDiff[];
  impact: {
    horizonDays: number;
    windowDays: number;
    assumptions: string[];
    products: ProductImpact[];
    totals: CurrencyTotal[];
  } | null;
  rules: RuleFinding[];
  blocking: boolean;
  notChecked: string[];
  reason?: string;
};
export type ExplanationSource = {
  id?: string;
  type: string;
  reference: string;
  revision?: number;
  digest?: string;
  scope: string;
};
export type ExplanationAction = {
  kind: string;
  tool?: string;
  at?: string;
  actor?: string | null;
  rationale: {
    origin: string;
    untrusted: boolean;
    text?: string;
    code?: string;
  };
  sourceIds: string[];
  sources?: ExplanationSource[];
};
export type Explanation = {
  version: number;
  actions: ExplanationAction[];
  sources: ExplanationSource[];
  omittedSources?: number;
  hiddenSources?: number;
};

/** Whether the stored simulation lets the merchant approve; proposals saved before simulation existed keep their review. */
export function approvable(simulation: Simulation | null | undefined) {
  return (
    !simulation || (simulation.status !== "failed" && !simulation.blocking)
  );
}

/** The rule findings worth showing first: violations, blocking ones before warnings. */
export function findings(rules: RuleFinding[]) {
  const rank = (r: RuleFinding) =>
    (r.outcome === "violated" ? 0 : 2) + (r.severity === "blocking" ? 0 : 1);
  return [...rules].sort((a, b) => rank(a) - rank(b));
}

/** The sources an action relied on, resolved against the explanation's catalogue; inline sources of applied actions come along. */
export function actionSources(e: Explanation, action: ExplanationAction) {
  return [
    ...action.sourceIds
      .map((id) => e.sources.find((s) => s.id === id))
      .filter((s): s is ExplanationSource => !!s),
    ...(action.sources ?? []),
  ];
}

/** A minor amount as a localized currency string. */
export function amount(
  locale: string,
  minor: number | null,
  currency: string,
  scale: number,
) {
  if (minor == null) return null;
  return new Intl.NumberFormat(locale, {
    style: "currency",
    currency,
    minimumFractionDigits: scale,
    maximumFractionDigits: scale,
  }).format(minor / 10 ** scale);
}
