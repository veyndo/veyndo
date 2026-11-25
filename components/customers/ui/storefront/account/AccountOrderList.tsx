/** Storefront adapter of the kit order-list part: live server states lead to a protected order detail. */
import { mediaImage } from "../../../../../frontend/src/shared/media/product-media";
import { contentText } from "../../../../../frontend/src/shared/i18n/content-language";
import { useAccountText } from "../../../../../frontend/src/shared/i18n/account-i18n";
import { useOperationsText } from "../../../../../frontend/src/shared/i18n/operations-i18n";
import { orderList } from "../../../../../frontend/src/kit/storefront";
import type { CustomerOrder } from "./account-types";
const List = orderList.Component;
/** A customer order as kit order data, with its state label in the buyer's language. */
export function usePartOrders(orders: CustomerOrder[]) {
  const { locale } = useAccountText(),
    { o } = useOperationsText();
  return orders.map((order) => ({
    id: order.id,
    number: order.orderNumber,
    date: order.createdAt,
    state:
      contentText(
        order.stateLabels?.[order.state] ?? {},
        locale,
        order.stateMainLocale ?? "en-GB",
      ) || o(order.state),
    total: order.cart.price.totalPrice,
    currency: order.cart.price.currency ?? "EUR",
    items: order.cart.lineItems.slice(0, 3).map((item) => ({
      label: item.label,
      quantity: item.quantity,
      image: mediaImage(item.media) || undefined,
    })),
  }));
}
export default function AccountOrderList({
  orders,
  onSelect,
  onShop,
}: {
  orders: CustomerOrder[];
  onSelect: (id: string) => void;
  onShop: () => void;
}) {
  const { locale } = useAccountText();
  return (
    <List
      locale={locale}
      orders={usePartOrders(orders)}
      onOpen={({ kind, id }) => (kind === "order" ? onSelect(id) : onShop())}
    />
  );
}
