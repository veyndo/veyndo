/** A shipping method with a free-shipping threshold says so, so a 0,00 € summary matches the 4,90 € card. */
import { render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import CheckoutMethods from "../../../components/checkout/ui/storefront/checkout/CheckoutMethods";
import { cart } from "./fixtures";

const method = (id: string, price: number, freeAbove: number | null) => ({
  id,
  name: id,
  description: "",
  active: true,
  countries: ["DE"],
  price,
  freeAbove,
  minDays: 2,
  maxDays: 4,
});

it("shows the free-shipping threshold only for methods that have one", () => {
  render(
    <CheckoutMethods
      cart={
        {
          ...cart,
          availableShippingMethods: [
            method("standard", 4.9, 150),
            method("express", 12.9, null),
          ],
        } as unknown as typeof cart
      }
      selection={{ ...cart.checkout } as never}
      onChange={vi.fn()}
      disabled={false}
    />,
  );
  // The unit environment runs the German interface.
  const hints = screen.getAllByText(/Kostenlos ab/);
  expect(hints).toHaveLength(1);
  expect(hints[0].textContent).toMatch(/150,00/);
});
