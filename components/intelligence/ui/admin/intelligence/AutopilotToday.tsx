/** Read-only autopilot summary: level per area, daily limits and today's native use; settings stay with the guardrail editor. */
import { useRead } from "../../../../../frontend/src/admin/shell/useRead";
import { Button } from "../../../../../frontend/src/kit/ui";
import { useEffect, useState } from "react";
import type { RequestFn } from "../../../../../frontend/src/admin/shell/studio-types";
import { useLocale } from "../../../../../frontend/src/shared/i18n/i18n";
import Icon from "../../../../../frontend/src/shared/ui/Icon";
import { useAutopilotText, type AutopilotWord } from "./autopilot-i18n";
import "./autopilot.css";
export type AutopilotStatus = {
  level: number;
  currency: string;
  scale: number;
  productsPerDay: number;
  productsUsed: number;
  maxDailyExposureMinor: number | null;
  exposureMinor: number;
  changes: {
    productId: string;
    name: string;
    currency: string;
    baselineMinor: number;
    currentMinor: number;
    stock: number;
    exposureMinor: number;
    counted: boolean;
  }[];
};
const fixedAreas: AutopilotWord[] = [
  "areaStock",
  "areaContent",
  "areaDesign",
  "areaRules",
];
export default function AutopilotToday({
  request,
  revision,
  canRevert = false,
}: {
  request: RequestFn;
  revision?: number;
  canRevert?: boolean;
}) {
  const t = useAutopilotText(),
    { locale } = useLocale();
  const [actionError, setError] = useState(""),
    [local, setLocal] = useState(0),
    [notice, setNotice] = useState("");
  const revert = async (productId?: string) => {
    setNotice("");
    try {
      const done = (await request(
        "/api/intelligence/autonomy.revert",
        { approve: true, ...(productId ? { productId } : {}) },
        "POST",
      )) as { reverted: string[]; skipped: { reason: string }[] };
      const kept = done.skipped.filter(
        (item) => item.reason === "changed_since",
      ).length;
      setNotice(
        [
          t("reverted", { count: done.reverted.length }),
          kept ? t("revertSkipped", { count: kept }) : "",
        ]
          .filter(Boolean)
          .join(" "),
      );
      setLocal((n) => n + 1);
    } catch (e) {
      setError((e as Error).message);
    }
  };
  // Read again after a settings revision or a local revert.
  const statusRead = useRead(request, "/api/intelligence/autonomy.status", {
    body: {},
    method: "POST",
    version: [revision, local],
  });
  const status = statusRead.data as Partial<AutopilotStatus> | undefined;
  const value =
    Array.isArray(status?.changes) && status.currency
      ? (status as AutopilotStatus)
      : undefined;
  const error =
    actionError || (statusRead.error as Error | undefined)?.message || "";
  useEffect(() => setError(""), [request, revision, local]);
  if (error)
    return (
      <p className="knowledge-alert" role="alert">
        {error}
      </p>
    );
  if (!value) return <p role="status">{t("loading")}</p>;
  const money = (minor: number, currency = value.currency) => {
    const amount = minor / 10 ** value.scale;
    try {
      return new Intl.NumberFormat(locale, {
        style: "currency",
        currency,
      }).format(amount);
    } catch {
      return `${amount.toFixed(value.scale)} ${currency}`;
    }
  };
  const share = (used: number, limit: number) =>
    `${Math.min(100, limit > 0 ? Math.round((used / limit) * 100) : 0)}%`;
  const active = value.level > 0;
  return (
    <section className="autopilot">
      <header>
        <span className="autopilot-emblem">
          <Icon name="gauge" size={20} />
        </span>
        <div>
          <h3>{t("title")}</h3>
          <p>{t("intro")}</p>
        </div>
      </header>
      <ul className="autopilot-areas">
        <li data-active={active}>
          <strong>{t("areaPrices")}</strong>
          <span>{t(active ? "level1" : "level0")}</span>
        </li>
        {fixedAreas.map((area) => (
          <li key={area}>
            <strong>{t(area)}</strong>
            <span>{t("level0")}</span>
          </li>
        ))}
      </ul>
      <p className="autopilot-note">{t("fixed")}</p>
      {active && (
        <div className="autopilot-today">
          <h4>{t("today")}</h4>
          <div className="autopilot-meter">
            <span>
              {t("products", {
                used: value.productsUsed,
                limit: value.productsPerDay,
              })}
            </span>
            <i>
              <b
                style={{
                  width: share(value.productsUsed, value.productsPerDay),
                }}
              />
            </i>
          </div>
          <div className="autopilot-meter">
            <span>
              {value.maxDailyExposureMinor == null
                ? t("exposureOpen", { used: money(value.exposureMinor) })
                : t("exposure", {
                    used: money(value.exposureMinor),
                    limit: money(value.maxDailyExposureMinor),
                  })}
            </span>
            {value.maxDailyExposureMinor != null && (
              <i>
                <b
                  style={{
                    width: share(
                      value.exposureMinor,
                      value.maxDailyExposureMinor,
                    ),
                  }}
                />
              </i>
            )}
          </div>
          <p className="autopilot-note">{t("exposureHint")}</p>
          {value.maxDailyExposureMinor != null && (
            <p className="autopilot-note">{t("guardHint")}</p>
          )}
          {notice && <p role="status">{notice}</p>}
          {canRevert &&
            value.changes.filter((c) => c.currentMinor !== c.baselineMinor)
              .length > 1 && (
              <Button
                type="button"
                className="autopilot-all"
                onClick={() => void revert()}
              >
                {t("revertAll")}
              </Button>
            )}
          {value.changes.length ? (
            <ul className="autopilot-changes">
              {value.changes.map((change) => (
                <li key={change.productId}>
                  <strong>{change.name}</strong>
                  <span>
                    {t("change", {
                      from: money(change.baselineMinor, change.currency),
                      to: money(change.currentMinor, change.currency),
                      stock: change.stock,
                      amount: money(change.exposureMinor, change.currency),
                    })}
                    {!change.counted && ` · ${t("otherCurrency")}`}
                  </span>
                  {change.currentMinor === change.baselineMinor ? (
                    <em>{t("atBaseline")}</em>
                  ) : (
                    canRevert && (
                      <Button
                        type="button"
                        onClick={() => void revert(change.productId)}
                      >
                        {t("revert")}
                      </Button>
                    )
                  )}
                </li>
              ))}
            </ul>
          ) : (
            <p className="autopilot-note">{t("none")}</p>
          )}
        </div>
      )}
    </section>
  );
}
