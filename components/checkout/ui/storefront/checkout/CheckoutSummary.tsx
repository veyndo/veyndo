/** Sticky order review (kit cart-summary part) presents authoritative totals and discounts beside the purchase action. */
import { mediaImage } from "../../../../../frontend/src/shared/media/product-media";
import type { Cart } from "../../../../../frontend/src/shared/api/shop-api";
import { useShopText } from "../../../../../frontend/src/shared/i18n/shop-i18n";
import { useWorkbenchText } from "../../../../../frontend/src/shared/i18n/workbench-i18n";
import { useCheckoutText } from "../../../../../frontend/src/shared/i18n/checkout-i18n";
import { useState } from "react";
import { cartSummary } from "../../../../../frontend/src/kit/storefront";
import { contentText } from "../../../../../frontend/src/shared/i18n/content-language";
import { getContentLocale } from "../../../../../frontend/src/shared/api/shop-api";
const Summary = cartSummary.Component;
import ConfigurationSummary from "../../../../../frontend/src/shared/apps/ConfigurationSummary";
import { configurationSnapshots } from "../../../../../frontend/src/shared/api/configuration-types";
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
        // Line totals and positionPrice already include the discounts; the
        // subtotal is shown before them so subtotal − discounts adds up.
        subtotal={beforeDiscounts(cart)}
        discounts={(cart.discounts ?? []).map((d) => ({
          id: d.id,
          // A title missing in this language inherits the stored main-language one.
          label:
            contentText(d.name, getContentLocale(), getContentLocale()) ||
            Object.values(d.name)[0] ||
            d.id,
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
/** Goods value before discounts, rounded to the currency's minor unit (no float drift). */
function beforeDiscounts(cart: Cart) {
  const unit = 10 ** (cart.price.currencyScale ?? 2);
  return (
    Math.round(
      cart.price.positionPrice * unit + (cart.discountTotal ?? 0) * unit,
    ) / unit
  );
}
