/** Product summary part: the purchase area of a product page with image, price, delivery and add-to-cart. */
import { useState } from "react";
import { z } from "zod";
import { Button } from "../ui";
import {
  amount,
  currency,
  definePart,
  locale,
  productId,
  url,
} from "./contract";
import { money, storefrontText } from "./storefront-i18n";

export const productSummary = definePart({
  id: "product-summary",
  version: 1,
  schema: z.object({
    locale,
    product: z.object({
      id: z.string().min(1).max(200),
      name: z.string().min(1).max(300),
      description: z.string().max(5000).optional(),
      image: url.optional(),
      price: amount,
      listPrice: amount.optional(),
      currency,
      stock: z.number().int().min(0),
      deliveryDays: z.number().int().min(0).max(365).optional(),
      minPurchase: z.number().int().min(1).default(1),
      purchaseSteps: z.number().int().min(1).default(1),
      maxPurchase: z.number().int().min(1).optional(),
    }),
    priceKind: z.enum(["gross", "net"]).default("gross"),
  }),
  config: z.object({ productId }),
  Component: function ProductSummary({
    locale,
    product: p,
    priceKind,
    onAddToCart,
  }) {
    const t = storefrontText(locale);
    const max = Math.min(p.maxPurchase ?? p.stock, p.stock);
    const [quantity, setQuantity] = useState(p.minPurchase);
    return (
      <section className="vy-sf-summary">
        <div className="vy-sf-summary-media">
          {p.image ? (
            <img src={p.image} alt={p.name} width="640" height="512" />
          ) : (
            <span className="vy-sf-placeholder">{t("noImage")}</span>
          )}
        </div>
        <div className="vy-sf-summary-body">
          <h1>{p.name}</h1>
          {p.description && <p>{p.description}</p>}
          <p className="vy-sf-price">
            <strong>{money(locale, p.currency, p.price)}</strong>
            {p.listPrice !== undefined && p.listPrice > p.price && (
              <s>{money(locale, p.currency, p.listPrice)}</s>
            )}
            <small>{t(priceKind)}</small>
          </p>
          <p className="vy-sf-stock" data-empty={p.stock === 0 || undefined}>
            {p.stock ? `${p.stock} ${t("available")}` : t("soldOut")}
            {p.deliveryDays !== undefined &&
              p.stock > 0 &&
              ` · ${t("delivery", { days: p.deliveryDays })}`}
          </p>
          <form
            className="vy-sf-buy"
            onSubmit={(event) => {
              event.preventDefault();
              onAddToCart?.({ productId: p.id, quantity });
            }}
          >
            <label>
              <span>{t("quantity")}</span>
              <input
                type="number"
                min={p.minPurchase}
                max={max || undefined}
                step={p.purchaseSteps}
                value={quantity}
                disabled={!p.stock}
                onChange={(event) => setQuantity(Number(event.target.value))}
              />
            </label>
            <Button type="submit" tone="primary" disabled={!p.stock}>
              {t("addToCart")}
            </Button>
          </form>
        </div>
      </section>
    );
  },
});
