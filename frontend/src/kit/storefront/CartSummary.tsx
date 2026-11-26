/** Cart summary part: priced lines with quantity controls, discounts, taxes and totals from the server cart. */
import { z } from "zod";
import { Button } from "../ui";
import { amount, currency, definePart, locale, url } from "./contract";
import { day, money, storefrontText } from "./storefront-i18n";

const count = z.number().int().min(1).max(100000);

export const cartSummary = definePart({
  id: "cart-summary",
  version: 1,
  schema: z.object({
    locale,
    currency,
    lines: z
      .array(
        z.object({
          id: z.string().min(1).max(200),
          label: z.string().min(1).max(300),
          quantity: count,
          unitPrice: amount.optional(),
          total: amount,
          url: url.optional(),
          image: url.optional(),
          minPurchase: count.default(1),
          purchaseSteps: count.default(1),
          maxPurchase: count.optional(),
        }),
      )
      .max(200),
    subtotal: amount.optional(),
    discounts: z
      .array(
        z.object({
          id: z.string().min(1).max(200),
          label: z.string().max(300),
          amount,
        }),
      )
      .max(50)
      .default([]),
    shipping: amount.optional(),
    tax: amount.optional(),
    total: amount,
    delivery: z
      .object({ earliest: z.string().max(40), latest: z.string().max(40) })
      .optional(),
    priceKind: z.enum(["gross", "net"]).default("gross"),
    /** Disables the quantity controls while the host waits for the server. */
    busy: z.boolean().default(false),
  }),
  config: z.object({ showCheckout: z.boolean().default(true) }),
  Component: function CartSummary(props) {
    const t = storefrontText(props.locale);
    const m = (value: number) => money(props.locale, props.currency, value);
    const items = props.lines.reduce((n, line) => n + line.quantity, 0);
    const change = props.onQuantity;
    return (
      <section className="vy-sf-cart" aria-label={t("summary")}>
        <h3 className="vy-sf-cart-title">
          {t("summary")} <span>{items}</span>
        </h3>
        {!props.lines.length && <p>{t("emptyCart")}</p>}
        <div className="vy-sf-cart-lines">
          {props.lines.map((line) => (
            <article className="vy-sf-cart-line" key={line.id}>
              <a
                href={line.url}
                onClick={() => props.onOpen?.({ kind: "line", id: line.id })}
              >
                {line.image && (
                  <img
                    src={line.image}
                    alt=""
                    width="48"
                    height="48"
                    loading="lazy"
                  />
                )}
                <strong>{line.label}</strong>
                <small>
                  {line.unitPrice !== undefined
                    ? `${m(line.unitPrice)} · ${t(props.priceKind)}`
                    : `${t("quantity")}: ${line.quantity}`}
                </small>
                {props.renderLineDetail?.(line.id)}
              </a>
              <strong>{m(line.total)}</strong>
              {change && (
                <>
                  <div className="vy-sf-stepper">
                    <button
                      type="button"
                      disabled={props.busy || line.quantity <= line.minPurchase}
                      aria-label={`${t("quantity")} − ${line.label}`}
                      onClick={() =>
                        change({
                          lineId: line.id,
                          quantity: line.quantity - line.purchaseSteps,
                        })
                      }
                    >
                      −
                    </button>
                    <span>{line.quantity}</span>
                    <button
                      type="button"
                      disabled={
                        props.busy ||
                        (line.maxPurchase !== undefined &&
                          line.quantity + line.purchaseSteps > line.maxPurchase)
                      }
                      aria-label={`${t("quantity")} + ${line.label}`}
                      onClick={() =>
                        change({
                          lineId: line.id,
                          quantity: line.quantity + line.purchaseSteps,
                        })
                      }
                    >
                      +
                    </button>
                  </div>
                  <button
                    type="button"
                    className="vy-sf-text-button"
                    disabled={props.busy}
                    onClick={() => change({ lineId: line.id, quantity: 0 })}
                  >
                    {t("remove")}
                  </button>
                </>
              )}
            </article>
          ))}
        </div>
        {props.slot?.("beforeTotals")}
        <dl className="vy-sf-totals">
          {props.subtotal !== undefined && (
            <div>
              <dt>{t("subtotal")}</dt>
              <dd>{m(props.subtotal)}</dd>
            </div>
          )}
          {props.discounts.map((d) => (
            <div key={d.id}>
              <dt>{d.label || t("discount")}</dt>
              <dd>−{m(d.amount)}</dd>
            </div>
          ))}
          {props.shipping !== undefined && (
            <div>
              <dt>{t("shipping")}</dt>
              <dd>{m(props.shipping)}</dd>
            </div>
          )}
          {props.tax !== undefined && (
            <div>
              <dt>{t("tax")}</dt>
              <dd>{m(props.tax)}</dd>
            </div>
          )}
          <div className="vy-sf-total">
            <dt>{t("total")}</dt>
            <dd>{m(props.total)}</dd>
          </div>
        </dl>
        {props.delivery && (
          <p className="vy-sf-delivery">
            {t("deliveryWindow", {
              from: day(props.locale, props.delivery.earliest),
              to: day(props.locale, props.delivery.latest),
            })}
          </p>
        )}
        {props.slot?.("afterTotals")}
        {props.onCheckout && props.lines.length > 0 && (
          <Button type="button" tone="primary" onClick={props.onCheckout}>
            {t("checkout")}
          </Button>
        )}
      </section>
    );
  },
});
