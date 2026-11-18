/** Scripted attack results for the selected private sandbox; outcomes come from native refusals, never from a model. */
import { useEffect, useState } from "react";
import type { RequestFn } from "../shell/studio-types";
import Icon from "../../shared/ui/Icon";
import { useLocale } from "../../shared/i18n/i18n";
import { useAttackText, type AttackWord } from "./scenario-attack-i18n";
import type { AttackProbe, AttackReport } from "./scenario-types";
const titles: Record<string, AttackWord> = {
  negative_quantity: "attackNegativeQuantity",
  unknown_product: "attackUnknownProduct",
  excess_quantity: "attackExcessQuantity",
  stale_cart: "attackStaleCart",
  price_injection: "attackPriceInjection",
  guessed_cart: "attackGuessedCart",
  forged_total: "attackForgedTotal",
  stale_review: "attackStaleReview",
  replayed_order: "attackReplayedOrder",
};
const findings: Record<string, AttackWord> = {
  accepted: "findingAccepted",
  limited: "findingLimited",
  refused_at_checkout: "findingRefusedAtCheckout",
  over_stock: "findingOverStock",
  ignored: "findingIgnored",
  total_changed: "findingTotalChanged",
  same_order: "findingSameOrder",
  replay_refused: "findingReplayRefused",
  duplicate_order: "findingDuplicateOrder",
  stock_double: "findingStockDouble",
  checkout_unavailable: "findingCheckoutUnavailable",
  failed_unexpectedly: "findingFailedUnexpectedly",
};
const outcomes: Record<AttackProbe["outcome"], AttackWord> = {
  held: "attackHeld",
  breached: "attackBreached",
  review: "attackReview",
};
import "./scenario-labs.css";
export default function ScenarioAttacks({
  request,
  onSaved,
}: {
  request: RequestFn;
  onSaved?: () => void;
}) {
  const t = useAttackText(),
    { locale } = useLocale();
  const [report, setReport] = useState<AttackReport>(),
    [busy, setBusy] = useState(false),
    [error, setError] = useState("");
  // A result belongs to the shop, sandbox and channel it ran in; the last saved one is shown again.
  useEffect(() => {
    let active = true;
    setReport(undefined);
    setError("");
    setBusy(false);
    request("/api/agent/scenario-probes")
      .then((value) => {
        const latest = (value as { latest?: AttackReport | null } | undefined)
          ?.latest;
        if (active && latest && Array.isArray(latest.probes)) setReport(latest);
      })
      .catch(() => {
        /* A missing history must not block a new run. */
      });
    return () => {
      active = false;
    };
  }, [request]);
  const start = async () => {
    setBusy(true);
    setError("");
    try {
      setReport(
        (await request(
          "/api/agent/scenario-probes",
          {},
          "POST",
        )) as AttackReport,
      );
      onSaved?.();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  return (
    <section className="scenario-attacks">
      <header>
        <span className="scenario-attack-emblem">
          <Icon name="lock" size={20} />
        </span>
        <div>
          <h3>{t("attackTitle")}</h3>
          <p>{t("attackIntro")}</p>
        </div>
        <button
          className="studio-secondary"
          disabled={busy}
          onClick={() => void start()}
        >
          {t(busy ? "attackRunning" : "attackStart")}
        </button>
      </header>
      {error && (
        <p className="scenario-error" role="alert">
          {error}
        </p>
      )}
      {report && (
        <>
          <p
            className={`scenario-attack-summary${report.summary.breached ? " is-breached" : ""}`}
            role="status"
          >
            <strong>
              {t("attackSummary")
                .replace("{held}", String(report.summary.held))
                .replace("{total}", String(report.probes.length))}
            </strong>{" "}
            {t("attackProduct").replace("{product}", report.product.name)}
          </p>
          <p className="scenario-attack-detail">
            {report.savedAt &&
              !Number.isNaN(Date.parse(report.savedAt)) &&
              `${t("attackSaved").replace("{time}", new Date(report.savedAt).toLocaleString(locale))} `}
            {t(
              report.configurationReview === "current"
                ? "attackCurrent"
                : report.configurationReview === "stale" ||
                    report.configurationReview === "changed_during_run"
                  ? "attackStale"
                  : "attackUnverified",
            )}
          </p>
          <ul className="scenario-attack-list">
            {report.probes.map((probe) => (
              <li key={probe.id} className={`is-${probe.outcome}`}>
                <span className={`scenario-state is-attack-${probe.outcome}`}>
                  {t(outcomes[probe.outcome] ?? "attackReview")}
                </span>
                <div>
                  <strong>
                    {titles[probe.id] ? t(titles[probe.id]) : probe.id}
                  </strong>
                  {probe.finding && findings[probe.finding] && (
                    <p>{t(findings[probe.finding])}</p>
                  )}
                  {probe.detail && (
                    <p className="scenario-attack-detail">
                      {t("attackRefusal")} <q>{probe.detail}</q>
                    </p>
                  )}
                </div>
              </li>
            ))}
          </ul>
        </>
      )}
    </section>
  );
}
