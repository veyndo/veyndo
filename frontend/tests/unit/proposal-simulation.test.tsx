/** The twin review shows diff, impact and rule findings side by side, refuses approval on blocking or failed simulations, names unsupported parts and marks model text as unverified. */
import { fireEvent, render, screen, within } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { LocaleScope } from "../../src/shared/i18n/i18n";
import ProposalCard from "../../src/admin/shell/ProposalCard";
import ProposalSimulation from "../../../components/studio/ui/admin/proposals/ProposalSimulation";
import {
  actionSources,
  amount,
  approvable,
  findings,
  type Explanation,
  type Simulation,
} from "../../../components/studio/ui/admin/proposals/proposal-simulation-model";
import { proposalReason } from "../../src/shared/history/history-model";

const projection = (
  units: number,
  gross: number | null,
  margin: number | null,
  stock: number,
) => ({
  units,
  grossRevenueMinor: gross,
  netRevenueMinor: gross,
  marginMinor: margin,
  endingStock: stock,
  stockoutDay: null,
});
const simulation = (patch: Partial<Simulation> = {}): Simulation => ({
  version: 1,
  status: "simulated",
  twin: "transactionalReplay",
  simulatedAt: "2026-11-27T10:00:00Z",
  parts: [{ kind: "productChanges", status: "simulated" }],
  diff: [
    {
      entity: "product",
      id: "lamp",
      field: "price",
      before: 119,
      after: 107.1,
    },
  ],
  impact: {
    horizonDays: 30,
    windowDays: 30,
    assumptions: ["priceElasticityNotModelled"],
    products: [
      {
        productId: "lamp",
        currency: "EUR",
        scale: 2,
        demand: "recorded",
        windowUnits: 12,
        windowOrders: 9,
        baseline: projection(12, 142800, 72000, 8),
        proposed: projection(12, 128520, 60000, 8),
        notes: [],
      },
    ],
    totals: [
      {
        currency: "EUR",
        scale: 2,
        baselineGrossMinor: 142800,
        proposedGrossMinor: 128520,
        deltaGrossMinor: -14280,
        baselineMarginMinor: 72000,
        proposedMarginMinor: 60000,
        deltaMarginMinor: -12000,
      },
    ],
  },
  rules: [
    {
      rule: "priceIndication",
      productId: "lamp",
      outcome: "passed",
      severity: "warning",
      codes: [],
    },
    {
      rule: "priceGuardrail",
      productId: "lamp",
      outcome: "passed",
      severity: "blocking",
      codes: [],
    },
  ],
  blocking: false,
  notChecked: ["promotions"],
  ...patch,
});
const explanation: Explanation = {
  version: 1,
  actions: [
    {
      kind: "toolCall",
      tool: "merchant.orders",
      rationale: {
        origin: "planner",
        untrusted: false,
        code: "readRequestedByModel",
      },
      sourceIds: ["s2"],
    },
    {
      kind: "proposal",
      rationale: {
        origin: "model",
        untrusted: true,
        text: "<b>Sales slowed</b>; corridor allows it",
      },
      sourceIds: ["s1", "s3"],
    },
  ],
  sources: [
    {
      id: "s1",
      type: "product",
      reference: "lamp",
      revision: 4,
      scope: "catalog.read",
    },
    {
      id: "s2",
      type: "operation",
      reference: "merchant.orders#1",
      digest: "d",
      scope: "assistant.use",
    },
    {
      id: "s3",
      type: "rule",
      reference: "aiPolicy",
      revision: 7,
      scope: "assistant.use",
    },
  ],
  hiddenSources: 2,
};
const message = (sim: Simulation | null): any => ({
  id: 1,
  role: "assistant",
  content: "Lower lamp",
  applied: false,
  data: {
    taskId: "task",
    preview: {
      catalogBefore: [
        { id: "lamp", name: "Arc Desk Light", price: 119, stock: 20 },
      ],
      proposal: {
        summary: "Lower lamp",
        changes: [{ product_id: "lamp", price: 107.1 }],
      },
      simulation: sim,
      explanation,
    },
  },
});

it("shows the replayed diff, projected impact and rule findings side by side and approves once", () => {
  const approve = vi.fn();
  const { container } = render(
    <LocaleScope locale="en-GB">
      <ProposalCard
        message={message(simulation())}
        onApply={approve}
        busy={false}
        canApply
      />
    </LocaleScope>,
  );
  const columns = container.querySelectorAll(".twin-column");
  expect([...columns].map((c) => c.getAttribute("data-column"))).toEqual([
    "diff",
    "impact",
    "rules",
  ]);
  expect(
    within(columns[0] as HTMLElement).getByText(/Gross price/),
  ).toBeVisible();
  expect(
    within(columns[1] as HTMLElement).getAllByText(/€1,285.20/).length,
  ).toBeGreaterThan(0);
  expect(within(columns[1] as HTMLElement).getByText(/-€142.80/)).toBeVisible();
  expect(
    within(columns[2] as HTMLElement).getByText(/AI price guardrails/),
  ).toBeVisible();
  expect(screen.getByText(/Not checked: promotions/)).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: /approve/i }));
  expect(approve).toHaveBeenCalledTimes(1);
});

it("refuses approval for a blocking violation or a failed replay and says why", () => {
  const blocked = simulation({
    blocking: true,
    rules: [
      {
        rule: "priceGuardrail",
        productId: "lamp",
        outcome: "violated",
        severity: "blocking",
        codes: ["corridorMinimum"],
      },
    ],
  });
  const { unmount } = render(
    <LocaleScope locale="en-GB">
      <ProposalCard
        message={message(blocked)}
        onApply={vi.fn()}
        busy={false}
        canApply
      />
    </LocaleScope>,
  );
  expect(screen.getByText(/cannot be approved/)).toBeVisible();
  expect(screen.getByText(/Below the corridor minimum/)).toBeVisible();
  expect(screen.getByRole("button", { name: /approve/i })).toBeDisabled();
  unmount();
  render(
    <LocaleScope locale="de-DE">
      <ProposalCard
        message={message(
          simulation({
            status: "failed",
            reason: "busy",
            impact: null,
            diff: [],
          }),
        )}
        onApply={vi.fn()}
        busy={false}
        canApply
      />
    </LocaleScope>,
  );
  expect(screen.getByText("Simulation fehlgeschlagen")).toBeVisible();
  expect(screen.getByText(/Frage den Agenten erneut/)).toBeVisible();
  expect(screen.queryByText(/Erwartete Wirkung/)).toBeNull();
});

it("names a proposal part the twin cannot simulate instead of showing a projection", () => {
  render(
    <LocaleScope locale="es-ES">
      <ProposalSimulation
        simulation={simulation({
          status: "unsupported",
          parts: [
            {
              kind: "experience",
              status: "unsupported",
              reason: "noConversionModel",
            },
          ],
          diff: [],
          impact: null,
          rules: [],
        })}
      />
    </LocaleScope>,
  );
  expect(screen.getAllByText(/No simulado/).length).toBeGreaterThan(0);
  expect(screen.getByText(/Presentación de la tienda/)).toBeVisible();
  expect(screen.getByText(/modelo de conversión/)).toBeVisible();
  expect(screen.queryByText(/Impacto previsto/)).toBeNull();
});

it("explains each action with unverified model text and its source references", () => {
  const { container } = render(
    <LocaleScope locale="en-GB">
      <ProposalSimulation
        explanation={explanation}
        names={{ lamp: "Arc Desk Light" }}
      />
    </LocaleScope>,
  );
  fireEvent.click(screen.getByText("Why the agent acted"));
  expect(screen.getByText("Unverified model text")).toBeVisible();
  // Model text renders as text, never as markup.
  expect(container.querySelector("blockquote b")).toBeNull();
  expect(screen.getByText(/<b>Sales slowed<\/b>/)).toBeVisible();
  expect(
    screen.getByText(/Requested by the model while planning/),
  ).toBeVisible();
  expect(screen.getByText("aiPolicy")).toBeVisible();
  expect(screen.getByText(/Revision 7/)).toBeVisible();
  expect(screen.getByText(/2 sources need permissions/)).toBeVisible();
});

it("derives approval, ordering, sources, money and history links from the stored shapes", () => {
  expect(approvable(null)).toBe(true);
  expect(approvable(simulation())).toBe(true);
  expect(approvable(simulation({ blocking: true }))).toBe(false);
  expect(approvable(simulation({ status: "failed" }))).toBe(false);
  const ordered = findings([
    {
      rule: "a",
      productId: "p",
      outcome: "passed",
      severity: "blocking",
      codes: [],
    },
    {
      rule: "b",
      productId: "p",
      outcome: "violated",
      severity: "warning",
      codes: [],
    },
    {
      rule: "c",
      productId: "p",
      outcome: "violated",
      severity: "blocking",
      codes: [],
    },
  ]);
  expect(ordered.map((r) => r.rule)).toEqual(["c", "b", "a"]);
  expect(
    actionSources(explanation, explanation.actions[1]).map((s) => s.reference),
  ).toEqual(["lamp", "aiPolicy"]);
  expect(amount("de-DE", 12345, "EUR", 2)).toContain("123,45");
  expect(amount("en-GB", null, "EUR", 2)).toBeNull();
  expect(proposalReason("agent-proposal:task-1")).toBe("task-1");
  expect(proposalReason("manual fix")).toBeNull();
});
