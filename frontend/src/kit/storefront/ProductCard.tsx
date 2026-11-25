/** Product card part: image, name, price with tax note and stock, linking to the product page. */
import { z } from "zod";
import {
  amount,
  currency,
  definePart,
  locale,
  productId,
  url,
} from "./contract";
import { money, storefrontText } from "./storefront-i18n";

export const productCard = definePart({
  id: "product-card",
  version: 1,
  schema: z.object({
    locale,
    product: z.object({
      id: z.string().min(1).max(200),
      name: z.string().min(1).max(300),
      url,
      image: url.optional(),
      description: z.string().max(2000).optional(),
      category: z.string().max(200).optional(),
      price: amount,
      currency,
      stock: z.number().int().min(0).optional(),
    }),
    priceKind: z.enum(["gross", "net"]).default("gross"),
  }),
  config: z.object({
    productId,
    priceKind: z.enum(["gross", "net"]).default("gross"),
  }),
  Component: function ProductCard({ locale, product: p, priceKind }) {
    const t = storefrontText(locale);
    return (
      <article className="vy-sf-card">
        <a
          className="vy-sf-card-media"
          href={p.url}
          aria-label={`${t("details")}: ${p.name}`}
        >
          {p.image ? (
            <img
              src={p.image}
              alt={p.name}
              loading="lazy"
              width="400"
              height="320"
            />
          ) : (
            <span className="vy-sf-placeholder">{t("noImage")}</span>
          )}
          {p.stock !== undefined && (
            <span
              className="vy-sf-stock"
              data-empty={p.stock === 0 || undefined}
            >
              {p.stock ? `${p.stock} ${t("available")}` : t("soldOut")}
            </span>
          )}
        </a>
        <div className="vy-sf-card-body">
          {p.category && <p className="vy-sf-kicker">{p.category}</p>}
          <a href={p.url}>
            <h3>{p.name}</h3>
          </a>
          {p.description && <p>{p.description}</p>}
          <p className="vy-sf-price">
            <strong>{money(locale, p.currency, p.price)}</strong>
            <small>{t(priceKind)}</small>
          </p>
        </div>
        <a className="vy-sf-card-link" href={p.url}>
          {t("details")}
          <span aria-hidden="true">→</span>
        </a>
      </article>
    );
  },
});
