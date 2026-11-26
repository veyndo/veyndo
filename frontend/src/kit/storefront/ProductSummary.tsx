/** Product summary part: the purchase area of a product page with price, variants, quantity prices, delivery and add-to-cart. */
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

const label = z.string().max(200);
const count = z.number().int().min(1).max(100000);

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
      minPurchase: count.default(1),
      purchaseSteps: count.default(1),
      maxPurchase: count.optional(),
      /** Above the name, for example brand and category. */
      kicker: z.string().max(300).optional(),
      sku: z.string().max(100).optional(),
      rating: z
        .object({
          average: z.number().min(0).max(5),
          count: z.number().int().min(0),
        })
        .optional(),
      /** Base price per reference unit (for example per litre). */
      reference: z.object({ price: amount, unit: label }).optional(),
    }),
    priceKind: z.enum(["gross", "net"]).default("gross"),
    /** Shows "plus shipping" beside the price and in the delivery note. */
    shippingExtra: z.boolean().default(false),
    /** Variant choices of the product family; labels are in the buyer's language. */
    groups: z
      .array(
        z.object({
          id: z.string().min(1).max(100),
          label,
          selected: label,
          options: z
            .array(
              z.object({
                value: z.string().min(1).max(100),
                label,
                available: z.boolean(),
                selected: z.boolean(),
                swatch: z.string().max(60).optional(),
              }),
            )
            .max(100),
        }),
      )
      .max(10)
      .default([]),
    moreVariants: z.enum(["none", "more", "loading"]).default("none"),
    /** Shows minimum, steps and maximum below the purchase button. */
    showRules: z.boolean().default(false),
    tiers: z
      .array(
        z.object({
          quantity: count,
          unitPrice: amount,
          discountPercent: z.number().min(0).max(100),
        }),
      )
      .max(20)
      .default([]),
    selectedTier: count.optional(),
    delivery: z
      .object({
        minDays: z.number().int().min(0).max(365),
        maxDays: z.number().int().min(0).max(365),
        method: label,
        country: label,
      })
      .optional(),
    /** A host-controlled quantity; without it the part keeps its own. */
    quantity: count.optional(),
    disabled: z.boolean().default(false),
  }),
  config: z.object({ productId }),
  Component: function ProductSummary(props) {
    const {
      locale: language,
      product: p,
      priceKind,
      onAddToCart,
      onQuantity,
      onSelect,
      onOpen,
    } = props;
    const t = storefrontText(language);
    const m = (value: number) => money(language, p.currency, value);
    const [own, setOwn] = useState(p.minPurchase);
    const quantity = props.quantity ?? own;
    const setQuantity = (value: number, settled = false) => {
      const next = Math.max(1, Math.min(100000, value || 1));
      if (props.quantity === undefined) setOwn(next);
      onQuantity?.({ lineId: p.id, quantity: next, settled });
    };
    const stars = Math.round(p.rating?.average ?? 0);
    return (
      <section className="vy-sf-summary">
        {p.image && (
          <div className="vy-sf-summary-media">
            <img src={p.image} alt={p.name} width="640" height="512" />
          </div>
        )}
        <div className="vy-sf-summary-body">
          {p.kicker && <p className="vy-sf-kicker">{p.kicker}</p>}
          <h1>{p.name}</h1>
          {p.rating && (
            <button
              type="button"
              className="vy-sf-rating"
              disabled={!onOpen}
              onClick={() => onOpen?.({ kind: "page", id: "reviews" })}
            >
              <span aria-hidden="true">
                {"★".repeat(stars)}
                {"☆".repeat(5 - stars)}
              </span>{" "}
              {p.rating.average.toFixed(1)} · {p.rating.count} {t("reviews")}
            </button>
          )}
          {p.description && (
            <p className="vy-sf-description">{p.description}</p>
          )}
          <div className="vy-sf-price">
            <strong>{m(p.price)}</strong>
            {p.listPrice !== undefined && p.listPrice > p.price && (
              <s>{m(p.listPrice)}</s>
            )}
            <small>
              {t(priceKind)}
              {props.shippingExtra && ` · ${t("shippingExtra")}`}
            </small>
          </div>
          {p.reference && (
            <small className="vy-sf-reference">
              {m(p.reference.price)} / {p.reference.unit}
            </small>
          )}
          {props.groups.map((group) => (
            <fieldset className="vy-sf-variants" key={group.id}>
              <legend>
                {group.label} <span>{group.selected}</span>
              </legend>
              <div>
                {group.options.map((option) => (
                  <button
                    type="button"
                    key={option.value}
                    aria-pressed={option.selected}
                    title={option.available ? undefined : t("invalidOption")}
                    disabled={props.disabled || !option.available || !onSelect}
                    onClick={() =>
                      onSelect?.({ group: group.id, value: option.value })
                    }
                  >
                    {option.swatch && (
                      <i className="vy-sf-swatch" data-swatch={option.swatch} />
                    )}{" "}
                    {option.label}
                  </button>
                ))}
              </div>
            </fieldset>
          ))}
          {props.moreVariants !== "none" && (
            <button
              type="button"
              className="vy-sf-more"
              disabled={props.moreVariants === "loading" || !onOpen}
              onClick={() => onOpen?.({ kind: "page", id: "variants" })}
            >
              {t(props.moreVariants === "loading" ? "loading" : "moreVariants")}
            </button>
          )}
          <p
            className="vy-sf-availability"
            data-empty={p.stock === 0 || undefined}
          >
            <i aria-hidden="true" />
            {p.stock ? `${p.stock} ${t("available")}` : t("soldOut")}
            {p.deliveryDays !== undefined &&
              p.deliveryDays > 0 &&
              p.stock > 0 &&
              ` · ${t("delivery", { days: p.deliveryDays })}`}
            {p.sku && <span>SKU {p.sku}</span>}
          </p>
          {!p.stock && props.groups.length > 0 && (
            <small className="vy-sf-reference">{t("soldHint")}</small>
          )}
          {props.slot?.("beforeBuy")}
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
                max={p.maxPurchase ?? 100000}
                step={p.purchaseSteps}
                value={quantity}
                disabled={props.disabled}
                onChange={(event) => setQuantity(Number(event.target.value))}
                onBlur={() => setQuantity(quantity, true)}
              />
            </label>
            <Button
              type="submit"
              tone="primary"
              disabled={props.disabled || !p.stock}
            >
              {t("addToCart")}
            </Button>
          </form>
          {props.showRules && (
            <small className="vy-sf-rules">
              {t("min")}: {p.minPurchase} · {t("steps")}: {p.purchaseSteps}
              {p.maxPurchase ? ` · ${t("max")}: ${p.maxPurchase}` : ""}
            </small>
          )}
          {props.tiers.length > 1 && (
            <div className="vy-sf-tiers">
              <h3>{t("tiers")}</h3>
              <table>
                <thead>
                  <tr>
                    <th>{t("quantity")}</th>
                    <th>{t("per")}</th>
                  </tr>
                </thead>
                <tbody>
                  {props.tiers.map((tier) => (
                    <tr
                      key={tier.quantity}
                      data-selected={
                        tier.quantity === props.selectedTier || undefined
                      }
                    >
                      <td>
                        {t("from")} {tier.quantity}
                      </td>
                      <td>
                        {m(tier.unitPrice)}
                        {tier.discountPercent > 0 && (
                          <span> −{tier.discountPercent}%</span>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
          {props.delivery && (
            <div className="vy-sf-delivery-note">
              <svg
                width="22"
                height="22"
                viewBox="0 0 24 24"
                aria-hidden="true"
              >
                <path
                  d="M3 7.5 12 3l9 4.5v9L12 21l-9-4.5zM3 7.5l9 4.5 9-4.5M12 12v9"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="1.5"
                />
              </svg>
              <div>
                <strong>{t("deliveryLabel")}</strong>
                <p>
                  {props.delivery.maxDays === 0
                    ? t("readyToday")
                    : `${props.delivery.minDays}–${props.delivery.maxDays} ${t("days")}`}{" "}
                  · {props.delivery.method}
                </p>
                <small>
                  {t("shippingExtra")} · {props.delivery.country}
                </small>
              </div>
            </div>
          )}
        </div>
      </section>
    );
  },
});
