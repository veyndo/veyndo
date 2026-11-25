/** Shop currency registry and inherited channel availability. Actions use saved revisions and native job APIs. */
import type { ApiPath } from "../../kit/api";
import { useRead, useSetRead } from "../shell/useRead";
import { Button, Field, Notice, Select, TextInput } from "../../kit/ui";
import { useState } from "react";
import type { CurrencyConfig } from "../../shared/api/shop-api";
import { useCurrencyText } from "../../shared/i18n/currency-i18n";
import type { InternationalProps } from "./CommerceSettings";
import "../../shared/styles/currencies.css";
export const defaultCurrencies: CurrencyConfig = {
  baseCurrency: "EUR",
  pricingCurrency: "EUR",
  defaultCurrency: "EUR",
  enabled: ["EUR"],
  definitions: [
    { code: "EUR", scale: 2, rate: "1.00000000", strategy: "automatic" },
  ],
  autoRefresh: false,
  rateSource: "manual",
  rateDate: null,
};
export default function CurrencySettings({
  config,
  patch,
  request,
  channel,
  revision,
  dirty,
  reload,
  canReadCatalogue = false,
  canWriteCatalogue = false,
}: {
  revision: number;
  dirty: boolean;
  reload: () => void;
  canReadCatalogue?: boolean;
  canWriteCatalogue?: boolean;
} & InternationalProps) {
  const { c, locale } = useCurrencyText();
  const cfg = config.currencies ?? defaultCurrencies;
  const currencies = useRead(request, "/store-api/currencies");
  const catalogue: { code: string; scale: number }[] =
    currencies.data?.catalogue ?? [];
  // Price jobs of the base shop poll every five seconds.
  const jobsPath = "/api/merchant/currencies/price-jobs";
  const jobRead = useRead(
    request,
    channel || !canReadCatalogue ? null : jobsPath,
    { refetchMs: 5000 },
  );
  const setRead = useSetRead(request);
  const jobs: {
    id: string;
    state: string;
    processed: number;
    data: { currency: string };
  }[] = jobRead.data?.jobs ?? [];
  const [actionError, setError] = useState(""),
    [busy, setBusy] = useState(false),
    [add, setAdd] = useState(""),
    [overwrite, setOverwrite] = useState(false);
  const update = (value: Partial<CurrencyConfig>) =>
    patch({ currencies: { ...cfg, ...value } });
  const error =
    actionError ||
    (currencies.error as Error | undefined)?.message ||
    (jobRead.error as Error | undefined)?.message ||
    "";
  const action = async (path: ApiPath, body: unknown, refresh = false) => {
    if (dirty) {
      setError(c("saveFirst"));
      return;
    }
    setBusy(true);
    setError("");
    try {
      await request(path, body);
      if (refresh) reload();
      else setRead(jobsPath, await request(jobsPath));
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  const label = (code: string) =>
    `${code} · ${new Intl.DisplayNames(locale, { type: "currency" }).of(code) ?? code}`;
  return (
    <div className="currency-settings">
      {error && <Notice tone="danger">{error}</Notice>}
      <div className="currency-settings-top">
        {!channel && (
          <label>
            {c("base")}
            <select
              value={cfg.baseCurrency}
              onChange={(e) => {
                const base = e.target.value;
                const rate = Number(
                  cfg.definitions.find((d) => d.code === base)?.rate ?? 1,
                );
                update({
                  baseCurrency: base,
                  definitions: cfg.definitions.map((d) => ({
                    ...d,
                    rate: (Number(d.rate) / rate).toFixed(8),
                  })),
                  rateSource: "manual",
                  rateDate: null,
                });
              }}
            >
              {cfg.definitions.map((d) => (
                <option key={d.code} value={d.code}>
                  {label(d.code)}
                </option>
              ))}
            </select>
            <small>{c("baseHint")}</small>
          </label>
        )}
        <Field label={c("default")}>
          <Select
            value={cfg.defaultCurrency}
            onChange={(e) => update({ defaultCurrency: e.target.value })}
          >
            {cfg.enabled.map((code) => (
              <option key={code} value={code}>
                {label(code)}
              </option>
            ))}
          </Select>
        </Field>
      </div>
      <p>
        {c("sourceCurrency")}: <strong>{cfg.pricingCurrency ?? "EUR"}</strong>
      </p>
      <h3>{c("available")}</h3>
      <div className="currency-chips">
        {cfg.definitions.map((d) => (
          <button
            type="button"
            key={d.code}
            aria-pressed={cfg.enabled.includes(d.code)}
            disabled={d.code === cfg.defaultCurrency}
            onClick={() =>
              update({
                enabled: cfg.enabled.includes(d.code)
                  ? cfg.enabled.filter((v) => v !== d.code)
                  : [...cfg.enabled, d.code],
              })
            }
          >
            {label(d.code)}
          </button>
        ))}
      </div>
      {!channel && (
        <>
          <div className="currency-add">
            <Field label={c("add")}>
              <Select value={add} onChange={(e) => setAdd(e.target.value)}>
                <option value="">—</option>
                {catalogue
                  .filter(
                    (d) => !cfg.definitions.some((v) => v.code === d.code),
                  )
                  .map((d) => (
                    <option key={d.code} value={d.code}>
                      {label(d.code)}
                    </option>
                  ))}
              </Select>
            </Field>
            <Button
              type="button"
              disabled={!add}
              onClick={() => {
                const d = catalogue.find((v) => v.code === add);
                if (d) {
                  update({
                    definitions: [
                      ...cfg.definitions,
                      { ...d, rate: "1.00000000", strategy: "automatic" },
                    ],
                    rateSource: "manual",
                    rateDate: null,
                  });
                  setAdd("");
                }
              }}
            >
              {c("add")}
            </Button>
          </div>
          <div className="currency-rate-cards">
            {cfg.definitions.map((d) => (
              <article key={d.code}>
                <header>
                  <strong>{label(d.code)}</strong>
                  <small>{d.scale}</small>
                </header>
                <Field label={c("rate")}>
                  <TextInput
                    inputMode="decimal"
                    value={d.rate}
                    disabled={d.code === cfg.baseCurrency}
                    onChange={(e) =>
                      update({
                        definitions: cfg.definitions.map((v) =>
                          v.code === d.code
                            ? { ...v, rate: e.target.value }
                            : v,
                        ),
                        rateSource: "manual",
                        rateDate: null,
                      })
                    }
                  />
                </Field>
                <Field label={c("strategy")}>
                  <Select
                    value={d.strategy}
                    onChange={(e) =>
                      update({
                        definitions: cfg.definitions.map((v) =>
                          v.code === d.code
                            ? {
                                ...v,
                                strategy: e.target.value as
                                  "fixed" | "automatic",
                              }
                            : v,
                        ),
                      })
                    }
                  >
                    <option value="automatic">{c("automatic")}</option>
                    <option value="fixed">{c("fixed")}</option>
                  </Select>
                </Field>
                {d.code !== cfg.baseCurrency && (
                  <Button
                    type="button"
                    disabled={cfg.enabled.includes(d.code)}
                    onClick={() =>
                      update({
                        definitions: cfg.definitions.filter(
                          (v) => v.code !== d.code,
                        ),
                      })
                    }
                  >
                    {c("remove")}
                  </Button>
                )}
                {d.strategy === "fixed" && (
                  <Button
                    tone="primary"
                    type="button"
                    disabled={busy || dirty || !canWriteCatalogue}
                    onClick={() =>
                      void action("/api/merchant/currencies/price-jobs", {
                        currency: d.code,
                        revision,
                        overwrite,
                      })
                    }
                  >
                    {c("generate")}
                  </Button>
                )}
              </article>
            ))}
          </div>
          <label className="currency-toggle">
            <input
              type="checkbox"
              checked={overwrite}
              onChange={(e) => setOverwrite(e.target.checked)}
            />
            {c("overwrite")}
          </label>
          <p>{c("generateHint")}</p>
          <div className="currency-feed">
            <strong>{c("source")}</strong>
            <span>
              {cfg.rateSource === "ecb" ? "ECB" : c("manual")} ·{" "}
              {cfg.rateDate ?? "—"}
            </span>
            <label className="currency-toggle">
              <input
                type="checkbox"
                checked={cfg.autoRefresh}
                onChange={(e) => update({ autoRefresh: e.target.checked })}
              />
              {c("autoRefresh")}
            </label>
            <Button
              tone="primary"
              type="button"
              disabled={busy || dirty || !canWriteCatalogue}
              onClick={() =>
                void action(
                  "/api/merchant/currencies/rates/refresh",
                  { revision },
                  true,
                )
              }
            >
              {c("refresh")}
            </Button>
            <p>{c("refreshHint")}</p>
          </div>
          {!!jobs.length && (
            <section>
              <h3>{c("jobs")}</h3>
              {jobs.map((j) => (
                <div className="currency-job" key={j.id}>
                  <b>{j.data.currency}</b>
                  <span>{c(j.state as "queued" | "completed" | "failed")}</span>
                  <span>
                    {c("processed")}: {j.processed}
                  </span>
                </div>
              ))}
            </section>
          )}
        </>
      )}
    </div>
  );
}
