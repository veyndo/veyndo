/** A signed-in checkout adopts the customer's default addresses, also ones added after sign-in, without overwriting typed input. */
import { render, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import CheckoutDetails from "../../../components/checkout/ui/storefront/checkout/CheckoutDetails";
import { shopApi } from "../../src/shared/api/shop-api";
import { cart } from "./fixtures";
vi.mock("../../src/shared/api/shop-api", async (original) => ({
  ...(await original<object>()),
  shopApi: vi.fn(),
}));
vi.mock(
  "../../../components/customers/ui/storefront/account/useSavedPaymentMethods",
  () => ({
    useSavedPaymentMethods: () => ({
      failed: false,
      supported: [],
      methods: [],
    }),
  }),
);

const home = {
  name: "Karla Kunde",
  firstName: "Karla",
  lastName: "Kunde",
  street: "Musterweg 5",
  postalCode: "80331",
  city: "München",
  country: "DE",
};
const book = {
  elements: [{ id: "home", address: home }],
  defaultBillingAddressId: "home",
  defaultShippingAddressId: "home",
  customerRevision: 2,
};

function mount(selection: Record<string, unknown>) {
  vi.mocked(shopApi).mockReset();
  vi.mocked(shopApi).mockResolvedValue(book);
  const onChange = vi.fn();
  const signedIn = { ...cart, customerId: "c-1", customerEmail: "k@x.test" };
  render(
    <CheckoutDetails
      cart={signedIn as typeof cart}
      selection={{ ...cart.checkout, ...selection } as never}
      onChange={onChange}
      onCart={vi.fn()}
      onSubmit={vi.fn()}
      busy={false}
    />,
  );
  return onChange;
}

it("selects the default address when the checkout has none", async () => {
  const onChange = mount({});
  await waitFor(() =>
    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({
        billingAddressId: "home",
        shippingAddressId: "home",
        address: home,
      }),
    ),
  );
});

it("keeps an address the customer already typed", async () => {
  const onChange = mount({
    billingAddress: { ...home, street: "Andere Straße 9" },
  });
  await waitFor(() =>
    expect(shopApi).toHaveBeenCalledWith("/store-api/account/addresses"),
  );
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(onChange).not.toHaveBeenCalledWith(
    expect.objectContaining({ billingAddressId: "home" }),
  );
});
