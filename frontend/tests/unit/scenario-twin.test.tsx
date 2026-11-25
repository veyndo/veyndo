/** The twin panel shows native readiness figures, links open steps to their owner and hides actions the role cannot use. */
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import ScenarioTwin from "../../../components/intelligence/ui/admin/scenarios/ScenarioTwin";
import { LocaleProvider } from "../../src/shared/i18n/i18n";
import type { RequestFn } from "../../src/admin/shell/studio-types";
import type { TwinReadiness } from "../../../components/intelligence/ui/admin/scenarios/scenario-types";
const twin = (change: Partial<TwinReadiness> = {}): TwinReadiness => ({
  sandbox: { id: "copy", name: "Winter copy" },
  catalogue: {
    products: 24,
    withStock: 20,
    withoutStock: 4,
    inactive: 0,
    visibleSample: 12,
  },
  checkout: { country: "DE", simulatedPayment: true, shipping: true },
  tests: { customerTests: 2 },
  ready: true,
  blockers: [],
  ...change,
});
const show = (
  request: unknown,
  access: string[],
  onOpen = vi.fn(),
  refresh = 0,
) => (
  <LocaleProvider>
    <ScenarioTwin
      request={request as RequestFn}
      refresh={refresh}
      access={access}
      onOpen={onOpen}
    />
  </LocaleProvider>
);
it("reads readiness once per refresh and shows a ready copy without repair actions", async () => {
  const request = vi.fn(async () => twin());
  const view = render(show(request, ["catalog.read"]));
  await waitFor(() => expect(screen.getByText(/Winter copy/)).toBeTruthy());
  expect(request).toHaveBeenCalledWith("/api/agent/scenario-readiness");
  expect(document.querySelectorAll(".is-done")).toHaveLength(4);
  expect(screen.queryAllByRole("button")).toHaveLength(0);
  expect(screen.getByText(/24/)).toBeTruthy();
  view.rerender(show(request, ["catalog.read"], vi.fn(), 1));
  await waitFor(() => expect(request).toHaveBeenCalledTimes(2));
});
it("names the zero-stock blocker and opens the product owner only with the right", async () => {
  const empty = twin({
    ready: false,
    blockers: ["no_stock"],
    catalogue: {
      products: 9,
      withStock: 0,
      withoutStock: 9,
      inactive: 0,
      visibleSample: 9,
    },
    tests: { customerTests: 0 },
  });
  const onOpen = vi.fn();
  const view = render(show(async () => empty, ["catalog.read"], onOpen));
  await waitFor(() =>
    expect(document.querySelectorAll(".is-open")).toHaveLength(3),
  );
  fireEvent.click(screen.getByRole("button"));
  expect(onOpen).toHaveBeenCalledWith("productData");
  view.unmount();
  render(show(async () => empty, []));
  await waitFor(() =>
    expect(document.querySelectorAll(".is-open")).toHaveLength(3),
  );
  expect(screen.queryAllByRole("button")).toHaveLength(0);
});
it("shows nothing for a foreign response shape and the native error on failure", async () => {
  const view = render(show(async () => ({ scenarios: [] }), []));
  await waitFor(() => expect(screen.getByRole("status")).toBeTruthy());
  expect(document.querySelector(".scenario-twin")).toBeNull();
  view.unmount();
  render(
    show(async () => {
      throw new Error("Open a private sandbox before simulating customers");
    }, []),
  );
  await waitFor(() =>
    expect(screen.getByRole("alert").textContent).toContain("private sandbox"),
  );
});
it("sets sandbox test stock with explicit approval, reloads and shows a current clean attack test as done", async () => {
  let stocked = false;
  const request = vi.fn(async (path: string, body?: unknown) => {
    if (path === "/api/agent/scenario-stock") {
      stocked = true;
      expect(body).toEqual({ quantity: 10, approve: true });
      return { updated: 9 };
    }
    return twin(
      stocked
        ? {
            catalogue: {
              products: 9,
              withStock: 9,
              withoutStock: 0,
              inactive: 0,
              visibleSample: 9,
            },
            tests: {
              customerTests: 1,
              attack: {
                summary: { held: 9, breached: 0, review: 0 },
                configurationReview: "current",
              },
            },
          }
        : {
            ready: false,
            blockers: ["no_stock"],
            catalogue: {
              products: 9,
              withStock: 0,
              withoutStock: 9,
              inactive: 0,
              visibleSample: 9,
            },
          },
    );
  });
  render(show(request, ["catalog.read", "catalog.write"]));
  await waitFor(() => expect(screen.getAllByRole("button")).toHaveLength(2));
  fireEvent.click(screen.getAllByRole("button")[0]);
  await waitFor(() =>
    expect(document.querySelectorAll(".is-done")).toHaveLength(5),
  );
  expect(document.body.textContent).toMatch(/9/);
  expect(screen.queryAllByRole("button")).toHaveLength(0);
});
