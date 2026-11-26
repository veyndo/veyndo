/** Dedicated completion page renders the accepted order snapshot and honest provider state, with no ID-only reads. */
import { mediaImage } from "../../../../../frontend/src/shared/media/product-media";
import { notifyCheckout, returnFromCheckout } from "./embedded-checkout";
import { useEffect, useRef } from "react";
import type { Order } from "../../../../../frontend/src/shared/api/shop-api";
import { useCheckoutText } from "../../../../../frontend/src/shared/i18n/checkout-i18n";
import { useShopText } from "../../../../../frontend/src/shared/i18n/shop-i18n";
import { useCustomerText } from "../../../../../frontend/src/shared/i18n/customer-i18n";
import AddressCard from "../../../../../frontend/src/shared/customer/AddressCard";
import Icon from "../../../../../frontend/src/shared/ui/Icon";
import PaymentSession from "./PaymentSession";
import CheckoutProgress from "./CheckoutProgress";
import OrderConfetti from "./OrderConfetti";
import ConfigurationSummary from "../../../../../frontend/src/shared/apps/ConfigurationSummary";
import { configurationSnapshots } from "../../../../../frontend/src/shared/api/configuration-types";
import { cartSummary, day } from "../../../../../frontend/src/kit/storefront";
const Summary = cartSummary.Component;
export default function OrderCompletion({
  order,
  onBack,
}: {
  order: Order;
  onBack: () => void;
}) {
  const { x } = useCheckoutText(),
    { s, locale } = useShopText(),
    { c } = useCustomerText();
  const heading = useRef<HTMLHeadingElement>(null);
  const snapshot = order.cart;
  useEffect(() => {
    notifyCheckout(order);
    heading.current?.focus();
    window.scrollTo({ top: 0, behavior: "instant" });
  }, [order.id]);
  return (
    <main className="order-completion">
      <OrderConfetti />
      <CheckoutProgress reviewed complete />
      <div className="order-completion-hero">
        <span className="order-success-mark">
          <Icon name="check" size={32} />
        </span>
        <p className="shop-kicker">{s("orderPlaced")}</p>
        <h1 ref={heading} tabIndex={-1}>
          {x("received")}
        </h1>
        <p>{x("receivedHint")}</p>
        <div className="order-reference">
          <span>{x("orderNumber")}</span>
          <strong>{order.orderNumber}</strong>
        </div>
      </div>
      <div className="order-completion-grid">
        <section className="completion-card">
          <Summary
            locale={locale}
            currency={snapshot.price.currency ?? "EUR"}
            priceKind={snapshot.price.taxStatus === "net" ? "net" : "gross"}
            busy={false}
            lines={snapshot.lineItems.map((item) => ({
              id: item.id,
              label: item.label,
              quantity: item.quantity,
              total: item.price.totalPrice,
              image: mediaImage(item.media) || undefined,
              minPurchase: 1,
              purchaseSteps: 1,
            }))}
            discounts={
              snapshot.discountTotal
                ? [
                    {
                      id: "discount",
                      label: "",
                      amount: snapshot.discountTotal,
                    },
                  ]
                : []
            }
            shipping={snapshot.shippingCosts.totalPrice}
            tax={snapshot.price.tax}
            total={snapshot.price.totalPrice}
            renderLineDetail={(id) => {
              const item = snapshot.lineItems.find((line) => line.id === id);
              return item ? (
                <ConfigurationSummary
                  configurations={configurationSnapshots(item)}
                />
              ) : null;
            }}
          />
        </section>
        <section className="completion-card">
          <h2>{c("shippingAddress")}</h2>
          <AddressCard address={snapshot.checkout.address} />
          <h2>{c("billingAddress")}</h2>
          <AddressCard address={snapshot.checkout.billingAddress} />
          {snapshot.customerEmail && <p>{snapshot.customerEmail}</p>}
          <h2>{s("payment")}</h2>
          {order.payment.attemptId ? (
            <PaymentSession
              autoRedirect
              id={order.payment.attemptId}
              token={snapshot.token}
            />
          ) : (
            <>
              <p>{s(order.payment.state)}</p>
              {order.payment.realMoneyCharged === false && (
                <p className="shop-disclosure">{x("test")}</p>
              )}
            </>
          )}
          {order.deliveries?.[0] && (
            <p className="delivery-estimate">
              {s("delivery")}:{" "}
              {day(locale, order.deliveries[0].deliveryDate.earliest)} —{" "}
              {day(locale, order.deliveries[0].deliveryDate.latest)}
            </p>
          )}
        </section>
      </div>
      <button
        className="shop-primary completion-back"
        onClick={() => {
          if (!returnFromCheckout()) onBack();
        }}
      >
        {x("back")}
        <Icon name="arrow" />
      </button>
    </main>
  );
}
