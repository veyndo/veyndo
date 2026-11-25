/** Purchase detail uses immutable addresses, current fulfillment, issued PDFs and paid order entitlements. */
import type { ApiPath } from "../../../../../frontend/src/kit/api";
import { mediaImage } from "../../../../../frontend/src/shared/media/product-media";
import { useEffect, useState } from "react";
import { contentText } from "../../../../../frontend/src/shared/i18n/content-language";
import { useAccountText } from "../../../../../frontend/src/shared/i18n/account-i18n";
import { useCustomerText } from "../../../../../frontend/src/shared/i18n/customer-i18n";
import { useOperationsText } from "../../../../../frontend/src/shared/i18n/operations-i18n";
import { useShopText } from "../../../../../frontend/src/shared/i18n/shop-i18n";
import { safeRichUrl } from "../../../../../frontend/src/shared/content/rich-document";
import AddressCard from "../../../../../frontend/src/shared/customer/AddressCard";
import { shopApi } from "../../../../../frontend/src/shared/api/shop-api";
import AccountDownloads from "./AccountDownloads";
import ConfigurationSummary from "../../../../../frontend/src/shared/apps/ConfigurationSummary";
import { configurationSnapshots } from "../../../../../frontend/src/shared/api/configuration-types";
import type { CustomerOrder, CustomerDownload } from "./account-types";
import { cartSummary } from "../../../../../frontend/src/kit/storefront";
const Summary = cartSummary.Component;
export default function AccountOrderDetail({
  id,
  onBack,
  onOrder,
  downloads,
  download,
  busy,
  run,
  onError,
  reload,
  loadOrder,
}: {
  id: string;
  onBack: () => void;
  onOrder: (id: string) => void;
  downloads: CustomerDownload[];
  download: (path: string) => Promise<void>;
  busy: boolean;
  run: (fn: () => Promise<void>) => Promise<void>;
  onError: (e: unknown) => void;
  reload?: () => Promise<void>;
  loadOrder?: () => Promise<CustomerOrder>;
}) {
  const { a, locale } = useAccountText(),
    { c } = useCustomerText(),
    { o } = useOperationsText(),
    { money: _currentMoney } = useShopText();
  const [order, setOrder] = useState<CustomerOrder>();
  const money = (n: number) =>
    new Intl.NumberFormat(locale, {
      style: "currency",
      currency: order?.cart.price.currency ?? "EUR",
    }).format(n);
  const [loading, setLoading] = useState(true);
  const path: ApiPath = `/store-api/account/orders/${encodeURIComponent(id)}`;
  useEffect(() => {
    let active = true;
    setOrder(undefined);
    setLoading(true);
    void (loadOrder ? loadOrder() : shopApi<CustomerOrder>(path))
      .then((next) => {
        if (active) setOrder(next);
      })
      .catch((e) => {
        if (active) onError(e);
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [path]);
  const stateLabel = (state: string) =>
    contentText(
      order?.stateLabels?.[state] ?? {},
      locale,
      order?.stateMainLocale ?? "en-GB",
    ) || o(state);
  const date = (value?: string) =>
    value
      ? new Date(value).toLocaleDateString(locale, {
          day: "numeric",
          month: "long",
          year: "numeric",
        })
      : "—";
  return (
    <section>
      <div className="account-section-toolbar">
        <button className="account-text-button" onClick={onBack}>
          ← {a("backOrders")}
        </button>
        <button
          className="shop-secondary"
          disabled={busy || loading}
          onClick={() =>
            void run(async () => {
              const [next] = await Promise.all([
                loadOrder ? loadOrder() : shopApi<CustomerOrder>(path),
                reload?.(),
              ]);
              setOrder(next);
            })
          }
        >
          {a("refresh")}
        </button>
      </div>
      {loading ? (
        <p role="status">{a("loading")}</p>
      ) : (
        order && (
          <>
            <div className="account-detail-heading">
              <div>
                <small>{date(order.createdAt)}</small>
                <h2>{order.orderNumber}</h2>
              </div>
              <strong>{money(order.cart.price.totalPrice)}</strong>
            </div>
            <div className="account-status-grid">
              <div>
                <small>{a("orderStatus")}</small>
                <span className="account-badge">{stateLabel(order.state)}</span>
              </div>
              <div>
                <small>{a("paymentStatus")}</small>
                <span className="account-badge">
                  {stateLabel(order.payment.state)}
                </span>
                <small>{order.payment.method?.name}</small>
              </div>
            </div>
            <section className="account-panel">
              <Summary
                locale={locale}
                currency={order.cart.price.currency ?? "EUR"}
                priceKind={
                  order.cart.price.taxStatus === "net" ? "net" : "gross"
                }
                busy={false}
                lines={order.cart.lineItems.map((item) => ({
                  id: item.id,
                  label: item.label,
                  quantity: item.quantity,
                  total: item.price.totalPrice,
                  image: mediaImage(item.media) || undefined,
                  minPurchase: 1,
                  purchaseSteps: 1,
                }))}
                subtotal={order.cart.price.positionPrice}
                discounts={[]}
                shipping={order.cart.shippingCosts.totalPrice}
                tax={order.cart.price.tax}
                total={order.cart.price.totalPrice}
                renderLineDetail={(lineId) => {
                  const item = order.cart.lineItems.find(
                    (l) => l.id === lineId,
                  );
                  return item ? (
                    <ConfigurationSummary
                      configurations={configurationSnapshots(item)}
                      language={locale}
                    />
                  ) : null;
                }}
              />
            </section>
            <section className="account-panel">
              <h3>{a("shipping")}</h3>
              {!order.deliveries?.length ? (
                <p>{a("digitalOrder")}</p>
              ) : (
                order.deliveries.map((delivery, index) => (
                  <article className="account-delivery" key={index}>
                    <div className="account-order-heading">
                      <strong>{delivery.shippingMethod.name}</strong>
                      <span className="account-badge">
                        {stateLabel(delivery.state)}
                      </span>
                    </div>
                    <p>
                      {a("deliveryWindow")}:{" "}
                      {date(delivery.deliveryDate?.earliest)} –{" "}
                      {date(delivery.deliveryDate?.latest)}
                    </p>
                    {delivery.trackingCode ? (
                      <p>
                        {a("trackingCode")}:{" "}
                        <strong>{delivery.trackingCode}</strong>
                      </p>
                    ) : (
                      <p className="account-muted">{a("noTracking")}</p>
                    )}
                    {delivery.trackingUrl?.startsWith("https://") &&
                      safeRichUrl(delivery.trackingUrl) && (
                        <a
                          className="account-tracking"
                          href={delivery.trackingUrl}
                          target="_blank"
                          rel="noopener noreferrer"
                        >
                          {a("trackShipment")} ↗
                        </a>
                      )}
                  </article>
                ))
              )}
            </section>
            <div className="customer-address-grid">
              <section className="account-panel">
                <h3>{c("billingAddress")}</h3>
                <AddressCard address={order.billingAddress} />
              </section>
              <section className="account-panel">
                <h3>{c("shippingAddress")}</h3>
                <AddressCard address={order.shippingAddress} />
              </section>
            </div>
            <section className="account-panel">
              <h3>{a("documents")}</h3>
              {!order.receipts?.length ? (
                <p className="account-muted">{a("noDocuments")}</p>
              ) : (
                order.receipts.map((receipt) => (
                  <div className="account-file" key={receipt.id}>
                    <div>
                      <strong>
                        {o(receipt.kind)} · {receipt.number}
                      </strong>
                      <small>{date(receipt.createdAt)}</small>
                    </div>
                    <button
                      className="shop-secondary"
                      disabled={busy}
                      onClick={() =>
                        void download(
                          receipt.pdfPath ||
                            `${path}/receipts/${encodeURIComponent(receipt.id)}/pdf`,
                        )
                      }
                    >
                      {a("download")}
                    </button>
                  </div>
                ))
              )}
            </section>
            {downloads.some((d) => d.orderId === id) && (
              <section className="account-panel">
                <h3>{a("downloads")}</h3>
                <AccountDownloads
                  downloads={downloads.filter((d) => d.orderId === id)}
                  download={download}
                  busy={busy}
                  onOrder={onOrder}
                />
              </section>
            )}
          </>
        )
      )}
    </section>
  );
}
