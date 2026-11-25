/** Account overview part: the signed-in buyer's name and recent orders, linking to their details. */
import { z } from "zod";
import { amount, currency, definePart, locale, url } from "./contract";
import { money, storefrontText } from "./storefront-i18n";

export const accountOverview = definePart({
  id: "account-overview",
  version: 1,
  schema: z.object({
    locale,
    name: z.string().max(300),
    orders: z
      .array(
        z.object({
          id: z.string().min(1).max(200),
          number: z.string().max(100),
          date: z.string().max(40),
          state: z.string().max(100),
          total: amount,
          currency,
          url: url.optional(),
        }),
      )
      .max(50),
  }),
  config: z.object({ orders: z.number().int().min(0).max(50).default(5) }),
  Component: function AccountOverview({ locale, name, orders }) {
    const t = storefrontText(locale);
    return (
      <section className="vy-sf-account">
        <h2>{t("account")}</h2>
        {name && <p className="vy-sf-account-name">{name}</p>}
        <h3>{t("orders")}</h3>
        {!orders.length ? (
          <p>{t("noOrders")}</p>
        ) : (
          <ul>
            {orders.map((order) => (
              <li key={order.id}>
                {order.url ? (
                  <a href={order.url}>{order.number}</a>
                ) : (
                  <span>{order.number}</span>
                )}
                <time dateTime={order.date}>
                  {new Date(order.date).toLocaleDateString(locale)}
                </time>
                <span>{order.state}</span>
                <strong>{money(locale, order.currency, order.total)}</strong>
              </li>
            ))}
          </ul>
        )}
      </section>
    );
  },
});
