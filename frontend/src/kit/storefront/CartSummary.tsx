/** Cart summary part: lines, subtotal, shipping and total from the server's priced cart. */
import { z } from "zod";
import { Button } from "../ui";
import { amount, currency, definePart, locale, url } from "./contract";
import { money, storefrontText } from "./storefront-i18n";

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
          quantity: z.number().int().min(1),
          total: amount,
          url: url.optional(),
        }),
      )
      .max(200),
    subtotal: amount,
    shipping: amount.optional(),
    total: amount,
    priceKind: z.enum(["gross", "net"]).default("gross"),
  }),
  config: z.object({ showCheckout: z.boolean().default(true) }),
  Component: function CartSummary(props) {
    const t = storefrontText(props.locale);
    const m = (value: number) => money(props.locale, props.currency, value);
    return (
      <section className="vy-sf-cart" aria-label={t("cart")}>
        <h2>{t("cart")}</h2>
        {!props.lines.length ? (
          <p>{t("emptyCart")}</p>
        ) : (
          <>
            <ul>
              {props.lines.map((line) => (
                <li key={line.id}>
                  {line.url ? (
                    <a href={line.url}>{line.label}</a>
                  ) : (
                    <span>{line.label}</span>
                  )}
                  <span>× {line.quantity}</span>
                  <strong>{m(line.total)}</strong>
                </li>
              ))}
            </ul>
            <dl>
              <dt>{t("subtotal")}</dt>
              <dd>{m(props.subtotal)}</dd>
              {props.shipping !== undefined && (
                <>
                  <dt>{t("shipping")}</dt>
                  <dd>{m(props.shipping)}</dd>
                </>
              )}
              <dt className="vy-sf-total">{t("total")}</dt>
              <dd className="vy-sf-total">
                {m(props.total)} <small>{t(props.priceKind)}</small>
              </dd>
            </dl>
            {props.onCheckout && (
              <Button type="button" tone="primary" onClick={props.onCheckout}>
                {t("checkout")}
              </Button>
            )}
          </>
        )}
      </section>
    );
  },
});
