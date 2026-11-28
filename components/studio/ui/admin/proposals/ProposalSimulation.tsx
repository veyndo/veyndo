/** Side-by-side twin review of an agent proposal: field diff, projected impact with its assumptions, rule findings, unsupported parts and the explanation of every agent action. Model text is shown as unverified data. */
import {
  Badge,
  Card,
  Notice,
  Stack,
  type Tone,
} from "../../../../../frontend/src/kit/ui";
import {
  actionSources,
  amount,
  findings,
  type Explanation,
  type RuleFinding,
  type Simulation,
} from "./proposal-simulation-model";
import { useProposalSimulationText } from "./proposal-simulation-i18n";

/** Columns wrap below each other on narrow screens instead of overflowing the card. */
const column = { flex: "1 1 14rem", minWidth: 0, overflowWrap: "anywhere" } as const;

function value(v: unknown) {
  return v == null ? "–" : String(v);
}

function statusTone(status: string): Tone {
  return status === "simulated"
    ? "positive"
    : status === "failed"
      ? "danger"
      : status === "notApplicable"
        ? "neutral"
        : "warning";
}

function ruleTone(r: RuleFinding): Tone {
  if (r.outcome === "passed") return "positive";
  if (r.outcome !== "violated") return "neutral";
  return r.severity === "blocking" ? "danger" : "warning";
}

export default function ProposalSimulation({
  simulation,
  explanation,
  names = {},
}: {
  simulation?: Simulation | null;
  explanation?: Explanation | null;
  names?: Record<string, string>;
}) {
  const { word, code, locale } = useProposalSimulationText();
  if (!simulation && !explanation) return null;
  const name = (id: string) => names[id] || id;
  const unsupported =
    simulation?.parts.filter((p) => p.status !== "simulated") ?? [];
  const impact = simulation?.impact;
  const money = (minor: number | null, currency: string, scale: number) =>
    amount(locale, minor, currency, scale) ?? word("unknown");
  return (
    <Stack as="section" gap="3" aria-label={word("title")}>
      {simulation && (
        <>
          <Stack direction="row" wrap align="center" gap="2">
            <strong>{word("title")}</strong>
            <Badge tone={statusTone(simulation.status)}>
              {code("status", simulation.status)}
            </Badge>
          </Stack>
          {simulation.status === "failed" && (
            <Notice tone="danger">{code("reason", simulation.reason)}</Notice>
          )}
          {simulation.blocking && (
            <Notice tone="danger">{word("blocked")}</Notice>
          )}
          {unsupported.map((p) => (
            <p key={p.kind} data-part={p.kind}>
              <b>{code("part", p.kind)}</b> · {code("status", p.status)}
              {p.reason && (
                <>
                  <br />
                  <small>{code("partReason", p.reason)}</small>
                </>
              )}
            </p>
          ))}
          {simulation.status !== "failed" &&
            simulation.parts.some((p) => p.status === "simulated") && (
              <Stack direction="row" wrap gap="3" align="start">
                <Card
                  title={word("diff")}
                  level={4}
                  className="twin-column"
                  data-column="diff"
                  style={column}
                >
                  {simulation.diff.length === 0 && <p>{word("noDiff")}</p>}
                  {simulation.diff.map((d) => (
                    <p key={`${d.id}-${d.field}`}>
                      <b>{name(d.id)}</b> · {code("field", d.field)}
                      <br />
                      {value(d.before)} → <strong>{value(d.after)}</strong>
                    </p>
                  ))}
                </Card>
                <Card
                  title={word("impact")}
                  level={4}
                  className="twin-column"
                  data-column="impact"
                  style={column}
                >
                  {impact && (
                    <Stack gap="2">
                      <small>
                        {word("horizon")
                          .replace("{days}", String(impact.horizonDays))
                          .replace("{window}", String(impact.windowDays))}
                      </small>
                      {impact.totals.map((t) => (
                        <p key={t.currency}>
                          {word("revenue")}:{" "}
                          {money(t.baselineGrossMinor, t.currency, t.scale)} →{" "}
                          <strong>
                            {money(t.proposedGrossMinor, t.currency, t.scale)}
                          </strong>{" "}
                          ({word("delta")}{" "}
                          {money(t.deltaGrossMinor, t.currency, t.scale)})
                          <br />
                          {word("margin")} · {word("delta")}:{" "}
                          {money(t.deltaMarginMinor, t.currency, t.scale)}
                        </p>
                      ))}
                      {impact.products.map((p) => (
                        <Stack gap="1" key={p.productId}>
                          <b>{name(p.productId)}</b>
                          {p.demand !== "recorded" && (
                            <small>{code("demand", p.demand)}</small>
                          )}
                          <table>
                            <thead>
                              <tr>
                                <th />
                                <th>{word("baseline")}</th>
                                <th>{word("proposed")}</th>
                              </tr>
                            </thead>
                            <tbody>
                              <tr>
                                <th>{word("units")}</th>
                                <td>{p.baseline.units}</td>
                                <td>{p.proposed.units}</td>
                              </tr>
                              <tr>
                                <th>{word("revenue")}</th>
                                <td>{money(p.baseline.grossRevenueMinor, p.currency, p.scale)}</td>
                                <td>{money(p.proposed.grossRevenueMinor, p.currency, p.scale)}</td>
                              </tr>
                              <tr>
                                <th>{word("margin")}</th>
                                <td>{money(p.baseline.marginMinor, p.currency, p.scale)}</td>
                                <td>{money(p.proposed.marginMinor, p.currency, p.scale)}</td>
                              </tr>
                              <tr>
                                <th>{word("endingStock")}</th>
                                <td>{p.baseline.endingStock}</td>
                                <td>{p.proposed.endingStock}</td>
                              </tr>
                            </tbody>
                          </table>
                          {p.proposed.stockoutDay != null && (
                            <small>
                              {word("stockout").replace(
                                "{day}",
                                String(p.proposed.stockoutDay),
                              )}
                            </small>
                          )}
                          {p.notes.map((n) => (
                            <small key={n}>{code("note", n)}</small>
                          ))}
                        </Stack>
                      ))}
                      <details>
                        <summary>{word("assumptions")}</summary>
                        <ul>
                          {impact.assumptions.map((a) => (
                            <li key={a}>{code("assumption", a)}</li>
                          ))}
                        </ul>
                      </details>
                    </Stack>
                  )}
                </Card>
                <Card
                  title={word("rules")}
                  level={4}
                  className="twin-column"
                  data-column="rules"
                  style={column}
                >
                  <Stack gap="2">
                    {findings(simulation.rules).map((r, index) => (
                      <p
                        key={`${r.rule}-${r.productId}-${index}`}
                        data-outcome={r.outcome}
                      >
                        <b>{code("rule", r.rule)}</b> · {name(r.productId)}
                        <br />
                        <Badge tone={ruleTone(r)}>
                          {code("outcome", r.outcome)}
                          {r.outcome === "violated" &&
                            ` · ${code("severity", r.severity)}`}
                        </Badge>
                        {r.codes.map((c) => (
                          <small key={c}>
                            <br />
                            {code("code", c)}
                          </small>
                        ))}
                      </p>
                    ))}
                    {simulation.notChecked.length > 0 && (
                      <small>
                        {word("notChecked").replace(
                          "{list}",
                          simulation.notChecked
                            .map((c) => code("check", c))
                            .join(", "),
                        )}
                      </small>
                    )}
                  </Stack>
                </Card>
              </Stack>
            )}
        </>
      )}
      {explanation && explanation.actions.length > 0 && (
        <details>
          <summary>{word("why")}</summary>
          <ol>
            {explanation.actions.map((action, index) => (
              <li key={index} data-action={action.kind}>
                <b>
                  {code("action", action.kind)}
                  {action.tool ? ` · ${action.tool}` : ""}
                </b>
                {action.rationale.text && (
                  <blockquote>
                    <Badge tone="warning">{word("untrusted")}</Badge>
                    <br />
                    {action.rationale.text}
                  </blockquote>
                )}
                {action.rationale.code && action.kind !== "simulation" && (
                  <p>{code("rationale", action.rationale.code)}</p>
                )}
                {actionSources(explanation, action).length > 0 && (
                  <ul aria-label={word("sources")}>
                    {actionSources(explanation, action).map((s, i) => (
                      <li key={`${s.type}-${s.reference}-${i}`}>
                        {code("source", s.type)} · <code>{s.reference}</code>
                        {s.revision != null &&
                          ` · ${word("revision").replace("{revision}", String(s.revision))}`}
                      </li>
                    ))}
                  </ul>
                )}
              </li>
            ))}
          </ol>
          {!!explanation.hiddenSources && (
            <small>
              {word("hiddenSources").replace(
                "{count}",
                String(explanation.hiddenSources),
              )}
            </small>
          )}
        </details>
      )}
    </Stack>
  );
}
