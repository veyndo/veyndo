/** Sticky order review (kit cart-summary part) presents authoritative totals and discounts beside the purchase action. */
import { mediaImage } from "../../shared/media/product-media";
import type { Cart } from "../../shared/api/shop-api";
import { useShopText } from "../../shared/i18n/shop-i18n";
import { useWorkbenchText } from "../../shared/i18n/workbench-i18n";
import { useCheckoutText } from "../../shared/i18n/checkout-i18n";
import { useState } from "react";
import { cartSummary } from "../../kit/storefront";
const Summary = cartSummary.Component;
import ConfigurationSummary from "../../shared/apps/ConfigurationSummary";
import { configurationSnapshots } from "../../shared/api/configuration-types";
export default function CheckoutSummary({
  cart,
  busy,
  reviewed,
  onCoupons,
  onQuantity,
  onClose,
}: {
  cart: Cart;
  busy: boolean;
  reviewed: boolean;
  onCoupons: (codes: string[]) => Promise<void>;
  onQuantity: (id: string, q: number) => void;
  onClose: () => void;
}) {
  const { locale, currency } = useShopText();
  const { w } = useWorkbenchText();
  const { x } = useCheckoutText();
  const [coupon, setCoupon] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const coupons = (
    <>
      <details className="checkout-coupon">
        <summary>{w("coupon")}</summary>
        <form
          onSubmit={async (e) => {
            e.preventDefault();
            setSaving(true);
            setError("");
            try {
              await onCoupons(coupon.trim() ? [coupon.trim()] : []);
            } catch (e) {
              setError((e as Error).message);
            } finally {
              setSaving(false);
            }
          }}
        >
          <label>
            {w("coupon")}
            <input
              value={coupon}
              disabled={busy || saving}
              maxLength={64}
              onChange={(e) => setCoupon(e.target.value)}
              placeholder={cart.couponCodes?.join(", ")}
            />
          </label>
          <button className="shop-secondary" disabled={busy || saving}>
            {w("applyCoupon")}
          </button>
        </form>
      </details>
      {cart.couponCodes?.length ? (
        <small>
          {cart.couponCodes.join(", ")} ·{" "}
          {cart.discounts?.length
            ? w("couponApplied")
            : w("couponNotApplicable")}
        </small>
      ) : null}
    </>
  );
  return (
    <aside className="checkout-summary">
      <Summary
        locale={locale}
        currency={currency}
        priceKind={cart.price.taxStatus === "net" ? "net" : "gross"}
        busy={busy}
        lines={cart.lineItems.map((item) => ({
          id: item.id,
          label: item.label,
          quantity: item.quantity,
          unitPrice: item.price.unitPrice,
          total: item.price.totalPrice,
          url: `#product/${encodeURIComponent(item.id)}`,
          image: mediaImage(item.media) || undefined,
          minPurchase: item.minPurchase,
          purchaseSteps: item.purchaseSteps,
          maxPurchase: item.maxPurchase ?? undefined,
        }))}
        subtotal={cart.price.positionPrice}
        discounts={(cart.discounts ?? []).map((d) => ({
          id: d.id,
          label: d.name[locale.slice(0, 2)] ?? d.id,
          amount: d.amount,
        }))}
        shipping={cart.shippingCosts.totalPrice}
        tax={cart.price.tax}
        total={cart.price.totalPrice}
        delivery={cart.deliveries[0]?.deliveryDate}
        onQuantity={({ lineId, quantity }) => onQuantity(lineId, quantity)}
        onOpen={onClose}
        renderLineDetail={(id) => {
          const item = cart.lineItems.find((line) => line.id === id);
          return item ? (
            <ConfigurationSummary
              configurations={configurationSnapshots(item)}
            />
          ) : null;
        }}
        slot={(name) => (name === "beforeTotals" ? coupons : null)}
      />
      <p className="checkout-note" role="status">
        {x(reviewed ? "reviewed" : "quoteHint")}
      </p>
      {cart.availablePaymentMethods.find(
        (p) => p.id === cart.checkout.paymentMethodId,
      )?.mode === "simulated" && <p className="shop-disclosure">{x("test")}</p>}
      {error && <p role="alert">{error}</p>}
    </aside>
  );
}
