/** GA4 discovery runs only for an installed app outside private browser journeys; absent apps cost no probe. */
import { render, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import ShopAnalytics from "../../../extensions/apps/google-analytics/storefront/ShopAnalytics";
import { AppSurfaceProvider } from "../../src/shared/apps/AppSurfaces";
import { shopApi } from "../../src/shared/api/shop-api";
vi.mock("../../src/shared/api/shop-api", () => ({ shopApi: vi.fn() }));

function mount(publicApps: string[]) {
  const registry = vi.fn().mockResolvedValue({ surfaces: [], publicApps });
  render(
    <AppSurfaceProvider public request={registry} scopeKey="unit-shop:default">
      <ShopAnalytics
        shop="unit-shop"
        channel="default"
        products={[]}
        bag={false}
      />
    </AppSurfaceProvider>,
  );
  return registry;
}

it.each(["sandbox", "preview"])(
  "does not discover GA4 in a %s browser journey",
  async (flag) => {
    history.replaceState(null, "", `/?shop=unit-shop&${flag}=1`);
    vi.mocked(shopApi).mockClear();
    vi.mocked(shopApi).mockResolvedValue({ measurementId: "G-SYNTHETIC" });
    const registry = mount(["google_analytics"]);
    await waitFor(() => expect(registry).toHaveBeenCalled());
    await waitFor(() => expect(shopApi).not.toHaveBeenCalled());
    expect(
      document.querySelector('script[src*="googletagmanager"]'),
    ).toBeNull();
  },
);

it("sends no tracking probe when the GA app is not installed", async () => {
  history.replaceState(null, "", "/?shop=unit-shop");
  vi.mocked(shopApi).mockClear();
  const registry = mount([]);
  await waitFor(() => expect(registry).toHaveBeenCalled());
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(shopApi).not.toHaveBeenCalled();
});

it("reads the tracking configuration once the registry lists the GA app", async () => {
  history.replaceState(null, "", "/?shop=unit-shop");
  vi.mocked(shopApi).mockClear();
  vi.mocked(shopApi).mockResolvedValue({});
  mount(["google_analytics"]);
  await waitFor(() =>
    expect(shopApi).toHaveBeenCalledWith(
      "/store-api/apps/google_analytics/actions/tracking",
      { salesChannel: "default" },
    ),
  );
});
