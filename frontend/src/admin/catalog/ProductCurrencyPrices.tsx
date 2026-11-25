/** Exact per-currency decimals live in the same revisioned product draft, including variants. */
import { useRead } from "../shell/useRead";
import { Button, Field, Notice, Select } from "../../kit/ui";
import "../../shared/styles/currencies.css";
import { convertDraftPrice } from "../../shared/money/fx-draft";
import { useEffect, useState } from "react";
import type { CurrencyDefinition } from "../../shared/api/shop-api";
import { useCurrencyText } from "../../shared/i18n/currency-i18n";
import type { ProductDraft } from "./catalog-model";
import type { RequestFn } from "../shell/studio-types";
export default function ProductCurrencyPrices({
  draft,
  onChange,
  request,
}: {
  draft: ProductDraft;
  onChange: (v: ProductDraft) => void;
  request: RequestFn;
}) {
  const { c, locale } = useCurrencyText();
  const [selected, setSelected] = useState("");
  const currencies = useRead(request, "/store-api/currencies");
  const list =
    currencies.data?.configuredCurrencies ??
    currencies.data?.availableCurrencies;
  const definitions: CurrencyDefinition[] = Array.isArray(list) ? list : [];
  const error =
    (currencies.error as Error | undefined)?.message ??
    (currencies.data && !Array.isArray(list) ? c("loading") : "");
  // The first fixed-rate currency (else the default) is selected once loaded.
  const loaded = currencies.data;
  useEffect(() => {
    if (!loaded || !Array.isArray(list)) return;
    setSelected(
      list.find((d: CurrencyDefinition) => d.strategy === "fixed")?.code ??
        loaded.defaultCurrency,
    );
  }, [loaded]);
  const prices = draft.extra.currencyPrices ?? {};
  const value = prices[selected];
  const patch = (field: string, text: string) => {
    const entry = { ...value, [field]: text };
    delete entry.generatedAt;
    onChange({
      ...draft,
      extra: {
        ...draft.extra,
        currencyPrices: { ...prices, [selected]: entry },
      },
    });
  };
  return (
    <section className="currency-feed">
      <Field label={c("priceCurrency")}>
        <Select
          value={draft.extra.priceCurrency ?? "EUR"}
          onChange={(e) => {
            const code = e.target.value;
            const target = definitions.find((v) => v.code === code);
            const source = definitions.find(
              (v) => v.code === (draft.extra.priceCurrency ?? "EUR"),
            );
            if (!target || !source) return;
            const commerce = { ...draft.commerce };
            for (const key of ["price", "listPrice", "regulationPrice"]) {
              if (commerce[key] != null)
                commerce[key] = convertDraftPrice(
                  commerce[key],
                  source,
                  target,
                );
            }
            onChange({
              ...draft,
              commerce,
              extra: { ...draft.extra, priceCurrency: code },
            });
          }}
        >
          {definitions.map((d) => (
            <option key={d.code} value={d.code}>
              {d.code}
            </option>
          ))}
        </Select>
      </Field>
      <h2>{c("prices")}</h2>
      {error && <Notice tone="danger">{error}</Notice>}
      <Field label={c("select")}>
        <Select value={selected} onChange={(e) => setSelected(e.target.value)}>
          {definitions.map((d) => (
            <option key={d.code} value={d.code}>
              {d.code} ·{" "}
              {new Intl.DisplayNames(locale, { type: "currency" }).of(d.code)}
            </option>
          ))}
        </Select>
      </Field>
      <p>
        {value
          ? definitions.find((d) => d.code === selected)?.strategy === "fixed"
            ? c("fixed")
            : c("automatic")
          : c("inherited")}
      </p>
      <div className="catalog-form-grid">
        {(["price", "listPrice", "regulationPrice"] as const).map(
          (field, index) => (
            <label key={field}>
              {c((["price", "list", "regulation"] as const)[index])}
              <input
                inputMode="decimal"
                value={value?.[field] ?? ""}
                placeholder={c("inherited")}
                onChange={(e) => patch(field, e.target.value)}
              />
            </label>
          ),
        )}
      </div>
      <Button
        type="button"
        disabled={!value}
        onClick={() => {
          const next = { ...prices };
          delete next[selected];
          onChange({
            ...draft,
            extra: { ...draft.extra, currencyPrices: next },
          });
        }}
      >
        {c("remove")}
      </Button>
    </section>
  );
}
