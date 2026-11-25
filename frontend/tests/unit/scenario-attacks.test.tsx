/** Attack results render native refusals, flag accepted attacks and never outlive their sandbox scope. */
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import ScenarioAttacks from "../../../components/intelligence/ui/admin/scenarios/ScenarioAttacks";
import { LocaleProvider } from "../../src/shared/i18n/i18n";
import type { RequestFn } from "../../src/admin/shell/studio-types";
import type { AttackProbe } from "../../../components/intelligence/ui/admin/scenarios/scenario-types";
const report = (breached: boolean) => ({
  id: "run",
  product: { id: "lamp", name: "Desk lamp" },
  summary: { held: breached ? 1 : 2, breached: breached ? 1 : 0, review: 0 },
  probes: [
    {
      id: "negative_quantity",
      outcome: "held",
      status: 400,
      detail: "number outside declared bounds",
    },
    breached
      ? { id: "forged_total", outcome: "breached", finding: "accepted" }
      : { id: "replayed_order", outcome: "held", finding: "same_order" },
  ] as AttackProbe[],
});
const show = (request: unknown) => (
  <LocaleProvider>
    <ScenarioAttacks request={request as RequestFn} />
  </LocaleProvider>
);
it("posts once to the probe operation and lists refusals with the shop's own answer", async () => {
  const request = vi.fn(
    async (_path: string, _body?: unknown, method?: string) =>
      method === "POST" ? report(false) : { latest: null },
  );
  render(show(request));
  await waitFor(() => expect(request).toHaveBeenCalledTimes(1));
  expect(screen.queryByRole("status")).toBeNull();
  fireEvent.click(screen.getByRole("button"));
  await waitFor(() =>
    expect(screen.getByRole("status").textContent).toContain("2"),
  );
  expect(request).toHaveBeenCalledTimes(2);
  expect(request).toHaveBeenCalledWith(
    "/api/agent/scenario-probes",
    {},
    "POST",
  );
  expect(screen.getByText("number outside declared bounds")).toBeTruthy();
  expect(screen.getByRole("status").className).not.toContain("is-breached");
  expect(screen.getAllByRole("listitem")).toHaveLength(2);
});
it("marks a run with an accepted attack and shows an unknown probe by its identifier", async () => {
  const value = report(true);
  value.probes.push({ id: "future_probe", outcome: "review" });
  const request = vi.fn(
    async (_path: string, _body?: unknown, method?: string) =>
      method === "POST" ? value : { latest: null },
  );
  render(show(request));
  fireEvent.click(screen.getByRole("button"));
  await waitFor(() =>
    expect(screen.getByRole("status").className).toContain("is-breached"),
  );
  expect(screen.getByText("future_probe")).toBeTruthy();
  expect(document.querySelector(".is-attack-breached")).toBeTruthy();
});
it("shows the native error and clears an old result when the sandbox scope changes", async () => {
  const failing = vi.fn(async () => {
    throw new Error("Open a private sandbox before simulating customers");
  });
  const posted = (value: unknown) =>
    vi.fn(async (_path: string, _body?: unknown, method?: string) =>
      method === "POST" ? value : { latest: null },
    );
  const view = render(show(failing));
  fireEvent.click(screen.getByRole("button"));
  await waitFor(() =>
    expect(screen.getByRole("alert").textContent).toContain("private sandbox"),
  );
  const working = posted(report(false));
  view.rerender(show(working));
  expect(screen.queryByRole("alert")).toBeNull();
  fireEvent.click(screen.getByRole("button"));
  await waitFor(() => expect(screen.getByRole("status")).toBeTruthy());
  view.rerender(show(posted(report(false))));
  expect(screen.queryByRole("status")).toBeNull();
});
it("shows the last saved report again and says when the configuration changed since", async () => {
  const saved = {
    ...report(false),
    savedAt: "2026-11-18T09:00:00Z",
    configurationReview: "stale" as const,
  };
  const onSaved = vi.fn();
  const request = vi.fn(
    async (_path: string, _body?: unknown, method?: string) =>
      method === "POST"
        ? { ...report(false), configurationReview: "current" }
        : { latest: saved },
  );
  render(
    <LocaleProvider>
      <ScenarioAttacks request={request as RequestFn} onSaved={onSaved} />
    </LocaleProvider>,
  );
  await waitFor(() => expect(screen.getByRole("status")).toBeTruthy());
  expect(document.body.textContent).toMatch(/changed after this test/);
  fireEvent.click(screen.getByRole("button"));
  await waitFor(() => expect(onSaved).toHaveBeenCalledTimes(1));
  expect(document.body.textContent).toMatch(/has not changed since this test/);
});
