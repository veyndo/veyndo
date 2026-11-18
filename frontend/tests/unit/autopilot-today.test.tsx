/** The autopilot summary shows the level per area and today's native use; it formats money in the shop currency and writes nothing. */
import { render, screen, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import AutopilotToday, {
  type AutopilotStatus,
} from "../../src/admin/intelligence/AutopilotToday";
import { LocaleProvider } from "../../src/shared/i18n/i18n";
import type { RequestFn } from "../../src/admin/shell/studio-types";
const status = (change: Partial<AutopilotStatus> = {}): AutopilotStatus => ({
  level: 1,
  currency: "EUR",
  scale: 2,
  productsPerDay: 20,
  productsUsed: 1,
  maxDailyExposureMinor: 10000,
  exposureMinor: 3690,
  changes: [
    {
      productId: "lamp",
      name: "Desk lamp",
      currency: "EUR",
      baselineMinor: 7123,
      currentMinor: 7000,
      stock: 30,
      exposureMinor: 3690,
      counted: true,
    },
  ],
  ...change,
});
const show = (request: unknown, revision = 1) => (
  <LocaleProvider>
    <AutopilotToday request={request as RequestFn} revision={revision} />
  </LocaleProvider>
);
it("reads the status again after a settings revision and shows limits, use and the changed product", async () => {
  const request = vi.fn(async () => status());
  const view = render(show(request));
  await waitFor(() => expect(screen.getByText("Desk lamp")).toBeTruthy());
  expect(request).toHaveBeenCalledWith(
    "/api/intelligence/autonomy.status",
    {},
    "POST",
  );
  expect(document.querySelector('[data-active="true"]')).toBeTruthy();
  const meters = [...document.querySelectorAll(".autopilot-meter b")].map(
    (bar) => (bar as HTMLElement).style.width,
  );
  expect(meters).toEqual(["5%", "37%"]);
  expect(document.body.textContent).toMatch(/36[.,]90/);
  expect(document.body.textContent).toMatch(/100[.,]00/);
  view.rerender(show(request, 2));
  await waitFor(() => expect(request).toHaveBeenCalledTimes(2));
});
it("shows level zero without today's figures and an open limit without a bar", async () => {
  const view = render(
    show(async () => status({ level: 0, changes: [], productsUsed: 0 })),
  );
  await waitFor(() =>
    expect(document.querySelector(".autopilot-areas")).toBeTruthy(),
  );
  expect(document.querySelector(".autopilot-today")).toBeNull();
  expect(document.querySelector('[data-active="true"]')).toBeNull();
  view.unmount();
  render(show(async () => status({ maxDailyExposureMinor: null })));
  await waitFor(() =>
    expect(document.querySelectorAll(".autopilot-meter")).toHaveLength(2),
  );
  expect(document.querySelectorAll(".autopilot-meter b")).toHaveLength(1);
});
it("shows nothing for a foreign response shape and the native error on failure", async () => {
  const view = render(show(async () => ({ data: {} })));
  await waitFor(() => expect(screen.getByRole("status")).toBeTruthy());
  expect(document.querySelector(".autopilot")).toBeNull();
  view.unmount();
  render(
    show(async () => {
      throw new Error("Workspace permission required");
    }),
  );
  await waitFor(() =>
    expect(screen.getByRole("alert").textContent).toContain("permission"),
  );
});
