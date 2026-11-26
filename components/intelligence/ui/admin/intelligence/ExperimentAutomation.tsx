/** Suggestion card, automation switches and decision note for experiments; the lifecycle and readout stay with the native owner. */
import { Button, Notice } from "../../../../../frontend/src/kit/ui";
import { useState } from "react";
import type { RequestFn } from "../../../../../frontend/src/admin/shell/studio-types";
import { useLocale } from "../../../../../frontend/src/shared/i18n/i18n";
import {
  useExperimentAutomationText,
  type ExperimentAutomationWord,
} from "./experiment-automation-i18n";
import "./autopilot.css";
export type Automation = { finish?: boolean; apply?: boolean };
type Suggestion = {
  design: Record<string, unknown> & { automation?: Automation };
  basis: {
    livePayments30Days: number;
    p95PaymentMinor: number;
    currentMode: string;
    currency: string;
    scale: number;
  };
  warnings: string[];
};
const warningWords: Record<string, ExperimentAutomationWord> = {
  no_live_payments: "warnNoLivePayments",
  low_volume: "warnLowVolume",
  already_running: "warnAlreadyRunning",
};
export function ExperimentSuggestion({
  request,
  disabled,
  onUse,
}: {
  request: RequestFn;
  disabled: boolean;
  onUse: (design: Suggestion["design"]) => void;
}) {
  const t = useExperimentAutomationText(),
    { locale } = useLocale();
  const [value, setValue] = useState<Suggestion>(),
    [error, setError] = useState(""),
    [busy, setBusy] = useState(false);
  const load = async () => {
    setBusy(true);
    setError("");
    try {
      const next = (await request(
        "/api/intelligence/experiment.suggest",
        {},
        "POST",
      )) as Suggestion;
      if (!next?.design || !next.basis) return;
      setValue(next);
      onUse({ ...next.design, title: t("suggestedName") });
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  const money = (minor: number, currency: string, scale: number) => {
    try {
      return new Intl.NumberFormat(locale, {
        style: "currency",
        currency,
      }).format(minor / 10 ** scale);
    } catch {
      return `${(minor / 10 ** scale).toFixed(scale)} ${currency}`;
    }
  };
  return (
    <section className="autopilot experiment-suggestion">
      <div>
        <h3>{t("suggestTitle")}</h3>
        <p>{t("suggestIntro")}</p>
      </div>
      <Button
        type="button"
        className="autopilot-all"
        disabled={disabled || busy}
        onClick={() => void load()}
      >
        {t("suggestButton")}
      </Button>
      {error && <Notice tone="danger">{error}</Notice>}
      {value && (
        <div role="status">
          <p>
            {t("suggestBasis", {
              count: value.basis.livePayments30Days,
              amount: money(
                value.basis.p95PaymentMinor,
                value.basis.currency,
                value.basis.scale,
              ),
              mode: value.basis.currentMode,
            })}
          </p>
          {value.warnings.map(
            (code) =>
              warningWords[code] && (
                <p key={code} className="experiment-warning">
                  {t(warningWords[code])}
                </p>
              ),
          )}
          <p>{t("suggestUsed")}</p>
        </div>
      )}
    </section>
  );
}
export function AutomationFields({
  value,
  onChange,
}: {
  value?: Automation;
  onChange: (next: Automation) => void;
}) {
  const t = useExperimentAutomationText();
  return (
    <>
      <label className="experiment-switch">
        <input
          type="checkbox"
          checked={!!value?.finish}
          onChange={(e) =>
            onChange({
              finish: e.target.checked,
              apply: e.target.checked && !!value?.apply,
            })
          }
        />
        <span>
          <strong>{t("autoFinish")}</strong>
          <small>{t("autoFinishHint")}</small>
        </span>
      </label>
      <label className="experiment-switch">
        <input
          type="checkbox"
          checked={!!value?.apply}
          disabled={!value?.finish}
          onChange={(e) => onChange({ finish: true, apply: e.target.checked })}
        />
        <span>
          <strong>{t("autoApply")}</strong>
          <small>{t("autoApplyHint")}</small>
        </span>
      </label>
    </>
  );
}
export function AutomationNote({
  automation,
  decision,
}: {
  automation?: Automation;
  decision?: string;
}) {
  const t = useExperimentAutomationText();
  if (!automation?.finish && !decision) return null;
  return (
    <p className="experiment-automation-note">
      {automation?.finish && <span>{t("badgeAuto")}</span>}
      {automation?.apply && <span>{t("badgeApply")}</span>}
      {decision === "published" && <strong>{t("decisionPublished")}</strong>}
      {decision === "kept" && <strong>{t("decisionKept")}</strong>}
    </p>
  );
}
