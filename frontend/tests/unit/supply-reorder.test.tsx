/** Supplier links are edited in the settings revision; a proposal shows signed offers and only an offer within the ceiling can be approved. */
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import SupplyReorder from "../../src/admin/channels/SupplyReorder";
import { LocaleProvider } from "../../src/shared/i18n/i18n";
import type { RequestFn } from "../../src/admin/shell/studio-types";
const settings = {
  revision: 3,
  data: {
    currencies: {
      pricingCurrency: "EUR",
      definitions: [{ code: "EUR", scale: 2 }],
    },
    supply: {
      maxOrderMinor: 50000,
      links: [
        {
          productId: "scarf",
          supplierShop: "north",
          supplierProductId: "scarf",
          reorderPoint: 40,
          reorderQuantity: 6,
        },
      ],
    },
  },
};
const offer = (total: string) => ({
  cartToken: "cart-capability",
  payloadBase64: "payload",
  signature: "signature",
  supplierStockEnough: true,
  payload: {
    totalMinor: total,
    currency: "EUR",
    currencyScale: 2,
    expiresAt: 1795000000,
  },
});
const proposal = {
  sufficient: 0,
  lines: [
    {
      productId: "scarf",
      name: "Soft Wool Scarf",
      stock: 30,
      reorderPoint: 40,
      quantity: 6,
      supplierShop: "north",
      sameCurrency: true,
      withinCeiling: true,
      offer: offer("35400"),
    },
    {
      productId: "bag",
      name: "Leather Tote",
      stock: 30,
      reorderPoint: 40,
      quantity: 5,
      supplierShop: "north",
      sameCurrency: true,
      withinCeiling: false,
      offer: offer("84500"),
    },
    { productId: "gone", supplierShop: "north", error: "product_missing" },
  ],
};
const transport = () =>
  vi.fn(async (path: string, body?: unknown) => {
    if (path === "/api/merchant/supply/proposal") return proposal;
    if (path === "/api/merchant/supply/approve") {
      expect(body).toEqual({
        productId: "scarf",
        payloadBase64: "payload",
        signature: "signature",
        cartToken: "cart-capability",
        approve: true,
      });
      return { checkoutPath: "/checkout?shop=north#checkout/ticket" };
    }
    return structuredClone(settings);
  });
const show = (request: unknown, rights: string[]) => (
  <LocaleProvider>
    <SupplyReorder request={request as RequestFn} rights={rights} />
  </LocaleProvider>
);
it("shows signed offers, blocks approval above the ceiling and links to the supplier checkout after approval", async () => {
  const request = transport();
  render(show(request, ["settings.read", "settings.write"]));
  fireEvent.click(
    await screen.findByRole("button", { name: "Get reorder proposal" }),
  );
  await screen.findByText("Soft Wool Scarf");
  const approve = screen.getAllByRole("button", {
    name: "Approve and open supplier checkout",
  }) as HTMLButtonElement[];
  expect(approve.map((button) => button.disabled)).toEqual([false, true]);
  expect(document.body.textContent).toMatch(/354[.,]00/);
  expect(
    screen.getByText("This product no longer exists in your shop."),
  ).toBeVisible();
  fireEvent.click(approve[0]);
  const link = await screen.findByRole("link", {
    name: /Open supplier checkout/,
  });
  expect(link.getAttribute("href")).toBe(
    "/checkout?shop=north#checkout/ticket",
  );
  expect(document.body.textContent).toContain("Nothing is ordered yet");
});
it("requires saving edited links before a proposal and hides everything without the settings right", async () => {
  const request = transport();
  const view = render(show(request, ["settings.read", "settings.write"]));
  const quantity = (await screen.findByDisplayValue("6")) as HTMLInputElement;
  fireEvent.change(quantity, { target: { value: "8" } });
  expect(
    (
      screen.getByRole("button", {
        name: "Get reorder proposal",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  expect(
    screen.getByText("Save your changes before asking for a proposal."),
  ).toBeVisible();
  view.unmount();
  const hidden = transport();
  render(show(hidden, ["catalog.read"]));
  await waitFor(() =>
    expect(document.querySelector(".supply-reorder")).toBeNull(),
  );
});
it("lets a read-only role see links and proposals but not change or approve them", async () => {
  render(show(transport(), ["settings.read"]));
  expect(
    await screen.findByText(/can view supplier links but not change/),
  ).toBeVisible();
  expect(
    (await screen.findByDisplayValue("6")).closest("fieldset")?.disabled,
  ).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "Get reorder proposal" }));
  await screen.findByText("Soft Wool Scarf");
  expect(
    (
      screen.getAllByRole("button", {
        name: "Approve and open supplier checkout",
      }) as HTMLButtonElement[]
    ).every((button) => button.disabled),
  ).toBe(true);
});
