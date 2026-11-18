/** The sovereign AI card reports the mode and the host of each model route; it writes nothing and stays hidden without a report. */
import { render, screen, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import SovereignAI, {
  type Sovereignty,
} from "../../src/admin/intelligence/SovereignAI";
import { LocaleProvider } from "../../src/shared/i18n/i18n";
import type { RequestFn } from "../../src/admin/shell/studio-types";
const report = (change: Partial<Sovereignty> = {}): Sovereignty => ({
  enabled: true,
  lockedByDeployment: false,
  holds: true,
  refusedProviders: ["openai", "anthropic"],
  routes: [
    {
      use: "text",
      configured: true,
      host: "ollama",
      selfHosted: true,
      admitted: true,
    },
    {
      use: "embeddings",
      configured: true,
      host: "ollama",
      selfHosted: true,
      admitted: true,
    },
    {
      use: "reranker",
      configured: false,
      host: null,
      selfHosted: null,
      admitted: false,
    },
    {
      use: "images",
      configured: true,
      host: "api.openai.com",
      selfHosted: false,
      admitted: false,
    },
  ],
  ...change,
});
const show = (request: unknown) => (
  <LocaleProvider>
    <SovereignAI request={request as RequestFn} />
  </LocaleProvider>
);
it("shows the mode, each route's host and the refused providers, and only reads", async () => {
  const request = vi.fn(async () => ({ providers: [], sovereignty: report() }));
  const view = render(show(request));
  await waitFor(() =>
    expect(view.container.querySelector('[data-state="on"]')).toBeTruthy(),
  );
  expect(request).toHaveBeenCalledTimes(1);
  expect(request).toHaveBeenCalledWith("/api/agent/providers");
  expect(view.container.querySelectorAll('li[data-own="true"]').length).toBe(2);
  expect(view.container.textContent).toContain("ollama");
  expect(view.container.textContent).toContain("OpenAI, Claude");
  // The image route names no external host as usable while the mode is on.
  expect(view.container.textContent).not.toContain("api.openai.com");
  expect(view.container.querySelector("button, input")).toBeNull();
});
it("marks a route that is not self-hosted as refused and says the mode does not hold", async () => {
  const broken = report({ holds: false });
  broken.routes[1] = {
    use: "embeddings",
    configured: true,
    host: "embed.example.com",
    selfHosted: false,
    admitted: false,
  };
  const view = render(show(async () => ({ sovereignty: broken })));
  await waitFor(() =>
    expect(
      view.container.querySelector('[data-state="onBroken"]'),
    ).toBeTruthy(),
  );
  expect(
    view.container.querySelector('li[data-refused="true"]')?.textContent,
  ).toContain("embed.example.com");
});
it("shows external hosts as allowed while the mode is off and nothing without a report", async () => {
  const off = report({ enabled: false, holds: false, refusedProviders: [] });
  off.routes[3].admitted = true;
  const view = render(show(async () => ({ sovereignty: off })));
  await waitFor(() =>
    expect(view.container.querySelector('[data-state="off"]')).toBeTruthy(),
  );
  expect(view.container.textContent).toContain("api.openai.com");
  expect(view.container.querySelector('li[data-refused="true"]')).toBeNull();
  const empty = render(show(async () => ({ providers: [] })));
  await waitFor(() => expect(empty.container.textContent).toBe(""));
  expect(screen.queryAllByRole("heading").length).toBe(1);
});
