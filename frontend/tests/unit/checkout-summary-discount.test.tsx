/** A coupon reads as subtotal − discount = total, labelled with the promotion title in the shopper's language. */
import { render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import CheckoutSummary from "../../../components/checkout/ui/storefront/checkout/CheckoutSummary";
import { LocaleProvider } from "../../src/shared/i18n/i18n";
import type { Cart } from "../../src/shared/api/shop-api";
import { cart } from "./fixtures";

it("shows the goods value before the discount and the promotion title", () => {
  history.replaceState(null, "", "/?shop=unit-shop&language=en-GB");
  const discounted: Cart = {
    ...cart,
    lineItems: [],
    couponCodes: ["WELCOME10"],
    discountTotal: 11.9,
    discounts: [
      {
        id: "welcome10",
        name: { "en-GB": "Welcome 10%", "de-DE": "Willkommen 10 %" },
        kind: "percentage",
        amount: 11.9,
      },
    ],
    price: {
      ...cart.price,
      currencyScale: 2,
      positionPrice: 107.1,
      totalPrice: 107.1,
      tax: 17.1,
    },
  };
  render(
    <CheckoutSummary
      cart={discounted}
      busy={false}
      reviewed
      onCoupons={vi.fn()}
      onQuantity={vi.fn()}
      onClose={vi.fn()}
    />,
    { wrapper: LocaleProvider },
  );
  const text = document.body.textContent ?? "";
  expect(text).toMatch(/119[.,]00/);
  expect(text).toMatch(/107[.,]10/);
  expect(screen.getByText("Welcome 10%")).toBeVisible();
  expect(screen.queryByText("welcome10")).toBeNull();
});
