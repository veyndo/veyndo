/** OverviewView renders verified shop state and typed user actions. */
import { Badge, Card } from "../../../../../frontend/src/kit/ui";
import { useReportText } from "../reports/report-i18n";
import StudioSlot from "../../../../../frontend/src/admin/shell/registry/StudioSlot";
import { useCatalogIntakeText } from "../../../../../frontend/src/shared/i18n/catalog-intake-i18n";
import { storefrontURL } from "../../../../../frontend/src/shared/api/shop-scope";
import { useLocale } from "../../../../../frontend/src/shared/i18n/i18n";
import Icon from "../../../../../frontend/src/shared/ui/Icon";
import ProductArt from "../../../../../frontend/src/shared/ui/ProductArt";
import type { Overview } from "../../../../../frontend/src/admin/shell/studio-types";
export function OverviewView({
  data,
  onIntent,
  onProduct,
}: {
  data: Overview;
  onIntent: (s: string) => void;
  onProduct: (id: string) => void;
}) {
  const intakeText = useCatalogIntakeText();
  const reportText = useReportText();
  const { t, date } = useLocale();
  const format = (n: number, currency = "EUR") =>
    new Intl.NumberFormat(data.locale, { style: "currency", currency }).format(
      n,
    );
  return (
    <div className="studio-page">
      <div className="page-intro">
        <h1>{reportText("title")}</h1>
        <p>{reportText("hint")}</p>
      </div>
      <StudioSlot location="admin.dashboard" context={{}} />
      <div className="overview-columns">
        <Card className="inventory-card">
          <div className="card-heading">
            <h2>{t("inventory")}</h2>
            <Badge>{t("api")}</Badge>
          </div>
          {data.products.map((p) => (
            <button
              className="inventory-row"
              key={p.id}
              onClick={() => onProduct(p.id)}
            >
              <div className="mini-art">
                <ProductArt id={p.id} />
              </div>
              <span>
                {p.name}
                <small>
                  {format(
                    p.price,
                    p.extra?.priceCurrency ?? data.productCurrency,
                  )}
                </small>
              </span>
              <b className={p.stock < 10 ? "stock-low" : ""}>{p.stock}</b>
              <Icon name="arrow" size={16} />
            </button>
          ))}
          {data.productsPagination.hasMore && (
            <p className="empty-note">
              {t("catalogPageNotice", { count: data.products.length })}{" "}
              <a href={storefrontURL(data.tenant)}>{t("storefront")} ↗</a>
            </p>
          )}
        </Card>
      </div>
      <Card>
        <div className="card-heading">
          <h2>{t("recentOrders")}</h2>
        </div>
        {data.orders.length ? (
          <div className="orders-table">
            {data.orders.map((o) => (
              <div className="order-row" key={o.id}>
                <span className="order-icon">
                  <Icon name="box" />
                </span>
                <b>{o.number}</b>
                <span>{date(o.time)}</span>
                <span className="channel-label">
                  {o.channel === "unknown" || !o.channel
                    ? t("unknown")
                    : o.channel.toUpperCase()}
                </span>
                <strong>{format(o.total, o.currency)}</strong>
              </div>
            ))}
          </div>
        ) : (
          <p className="empty-note">{t("emptyOrders")}</p>
        )}
      </Card>
      <div className="overview-columns">
        <Card>
          <div className="card-heading">
            <h2>{t("activity")}</h2>
          </div>
          {data.activity.map((a, i) => (
            <div className="activity-row" key={a.id + i}>
              <span className="activity-dot" />
              <div>
                {a.kind === "order"
                  ? t("activityOrder")
                  : a.kind === "approved"
                    ? t("activityApproved")
                    : t("activityProposal")}
                <small>{date(a.time)}</small>
              </div>
            </div>
          ))}
          {!data.activity.length && <p>{t("noActivity")}</p>}
        </Card>
        <Card className="next-actions">
          <h2>{t("actions")}</h2>
          <button onClick={() => onIntent(t("askStock"))}>
            <Icon name="box" />
            {t("inventory")}
            <Icon name="arrow" />
          </button>
          <button onClick={() => onIntent(t("learningPrompt"))}>
            <Icon name="pulse" />
            {t("policy")}
            <Icon name="arrow" />
          </button>
          <button onClick={() => onIntent(intakeText("knowledgePrompt"))}>
            <Icon name="graph" />
            {t("productKnowledge")}
            <Icon name="arrow" />
          </button>
        </Card>
      </div>
    </div>
  );
}
