/** Account home (kit account-overview part) connects recent purchases and default addresses to their management views. */
import { useAccountText } from "../../../../../frontend/src/shared/i18n/account-i18n";
import { useCustomerText } from "../../../../../frontend/src/shared/i18n/customer-i18n";
import AddressCard from "../../../../../frontend/src/shared/customer/AddressCard";
import { usePartOrders } from "./AccountOrderList";
import { accountOverview } from "../../../../../frontend/src/kit/storefront";
import type {
  AccountProfile,
  AccountPage,
  CustomerOrder,
} from "./account-types";
const Overview = accountOverview.Component;
export default function AccountOverview({
  profile,
  orders,
  downloads,
  onPage,
  onOrder,
  onShop,
}: {
  profile: AccountProfile;
  orders: CustomerOrder[];
  downloads: number;
  onPage: (page: AccountPage) => void;
  onOrder: (id: string) => void;
  onShop: () => void;
}) {
  const { a, locale } = useAccountText(),
    { c } = useCustomerText();
  const book = profile.addresses;
  const partOrders = usePartOrders(orders);
  return (
    <Overview
      locale={locale}
      name={profile.profile.firstName || profile.profile.name || profile.email}
      counts={{
        orders: profile.orderCount ?? orders.length,
        addresses: book?.elements.length ?? 0,
        downloads,
      }}
      orders={partOrders}
      recent={2}
      onOpen={({ kind, id }) =>
        kind === "order"
          ? onOrder(id)
          : id === "shop"
            ? onShop()
            : onPage(id as AccountPage)
      }
      slot={(name) =>
        name === "end" ? (
          <>
            <div className="account-section-toolbar">
              <h3>{a("defaultAddresses")}</h3>
              <button
                className="account-text-button"
                onClick={() => onPage("addresses")}
              >
                {a("manageAddresses")} →
              </button>
            </div>
            <div className="customer-address-grid">
              {(
                [
                  ["billingAddress", book?.defaultBillingAddressId],
                  ["shippingAddress", book?.defaultShippingAddressId],
                ] as const
              ).map(([kind, id]) => (
                <section className="account-panel" key={kind}>
                  <h4>{c(kind)}</h4>
                  {id ? (
                    <AddressCard
                      address={
                        book?.elements.find((entry) => entry.id === id)?.address
                      }
                    />
                  ) : (
                    <button
                      className="account-text-button"
                      onClick={() => onPage("addresses")}
                    >
                      {c("addAddress")} →
                    </button>
                  )}
                </section>
              ))}
            </div>
          </>
        ) : null
      }
    />
  );
}
