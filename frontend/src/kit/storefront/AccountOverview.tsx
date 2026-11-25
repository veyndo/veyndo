/** Account parts: the account home (greeting, counts, recent orders) and the order list with live server states. */
import { z } from "zod";
import type { PartEvents } from "./contract";
import { Button } from "../ui";
import { amount, currency, definePart, locale, url } from "./contract";
import { money, storefrontText } from "./storefront-i18n";

const order = z.object({
  id: z.string().min(1).max(200),
  number: z.string().max(100),
  /** ISO date of the order, shown in the buyer's language. */
  date: z.string().max(40).optional(),
  /** The server's state label, already in the buyer's language. */
  state: z.string().max(100),
  total: amount,
  currency,
  items: z
    .array(
      z.object({
        label: z.string().max(300),
        quantity: z.number().int().min(1),
        image: url.optional(),
      }),
    )
    .max(3)
    .default([]),
});
type Order = z.infer<typeof order>;

function Orders({
  locale: language,
  orders,
  onOpen,
}: { locale: string; orders: Order[] } & Pick<PartEvents, "onOpen">) {
  const t = storefrontText(language);
  if (!orders.length)
    return (
      <div className="vy-sf-orders-empty">
        <span aria-hidden="true">◇</span>
        <h3>{t("emptyOrders")}</h3>
        <p>{t("emptyOrdersHint")}</p>
        <Button
          type="button"
          tone="primary"
          onClick={() => onOpen?.({ kind: "page", id: "shop" })}
        >
          {t("continueShopping")}
        </Button>
      </div>
    );
  const date = (value?: string) => {
    const parsed = value ? new Date(value) : undefined;
    return parsed && !Number.isNaN(parsed.getTime())
      ? parsed.toLocaleDateString(language, {
          day: "numeric",
          month: "long",
          year: "numeric",
        })
      : "";
  };
  return (
    <div className="vy-sf-orders">
      {orders.map((o) => (
        <button
          type="button"
          className="vy-sf-order"
          key={o.id}
          onClick={() => onOpen?.({ kind: "order", id: o.id })}
          aria-label={`${t("viewOrder")} ${o.number}`}
        >
          <div className="vy-sf-order-heading">
            <div>
              <strong>{o.number}</strong>
              <small>{date(o.date)}</small>
            </div>
            <strong>{money(language, o.currency, o.total)}</strong>
          </div>
          {o.items.length > 0 && (
            <div className="vy-sf-order-items">
              {o.items.map((item, index) => (
                <span key={index}>
                  {item.image && <img src={item.image} alt="" loading="lazy" />}
                  <span>
                    {item.quantity} × {item.label}
                  </span>
                </span>
              ))}
            </div>
          )}
          <div className="vy-sf-order-footer">
            <span className="vy-sf-badge">{o.state}</span>
            <span>
              {t("viewOrder")} <span aria-hidden="true">↗</span>
            </span>
          </div>
        </button>
      ))}
    </div>
  );
}

export const orderList = definePart({
  id: "order-list",
  version: 1,
  schema: z.object({ locale, orders: z.array(order).max(100) }),
  config: z.object({}),
  Component: function OrderList({ locale: language, orders, onOpen }) {
    return <Orders locale={language} orders={orders} onOpen={onOpen} />;
  },
});

export const accountOverview = definePart({
  id: "account-overview",
  version: 1,
  schema: z.object({
    locale,
    name: z.string().max(300),
    counts: z.object({
      orders: z.number().int().min(0),
      addresses: z.number().int().min(0),
      downloads: z.number().int().min(0),
    }),
    orders: z.array(order).max(50),
    /** How many recent orders the home shows. */
    recent: z.number().int().min(0).max(50).default(2),
  }),
  config: z.object({ recent: z.number().int().min(0).max(50).default(2) }),
  Component: function AccountOverview(props) {
    const t = storefrontText(props.locale);
    const open = (id: string) => props.onOpen?.({ kind: "page", id });
    return (
      <section className="vy-sf-account" aria-label={t("account")}>
        <div className="vy-sf-account-welcome">
          <small>{t("welcome")}</small>
          <h2>{props.name}</h2>
          <p>{t("intro")}</p>
        </div>
        <div className="vy-sf-account-metrics">
          {(["orders", "addresses", "downloads"] as const).map((page) => (
            <button type="button" key={page} onClick={() => open(page)}>
              <strong>{props.counts[page]}</strong>
              <span>{t(page)}</span>
              <span aria-hidden="true">↗</span>
            </button>
          ))}
        </div>
        <div className="vy-sf-account-toolbar">
          <h3>{t("recentOrders")}</h3>
          <button
            type="button"
            className="vy-sf-link-button"
            onClick={() => open("orders")}
          >
            {t("allOrders")} →
          </button>
        </div>
        <Orders
          locale={props.locale}
          orders={props.orders.slice(0, props.recent)}
          onOpen={props.onOpen}
        />
        {props.slot?.("end")}
      </section>
    );
  },
});
