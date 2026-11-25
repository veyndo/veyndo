/** Search the canonical merchant catalog; unit permissions persist through the native revision-bound catalog API. */
import { Field, Notice, TextInput } from "../../kit/ui";
import { useState } from "react";
import type { RequestFn } from "../../shared/api/types";
import { useRead } from "../shell/useRead";
import { useDebouncedValue } from "../../kit/query";
import { useB2BText } from "../../shared/i18n/b2b-i18n";
export default function UnitCatalog({
  request,
  ids,
  onChange,
  disabled,
}: {
  request: RequestFn;
  ids: string[];
  onChange: (ids: string[]) => void;
  disabled: boolean;
}) {
  const t = useB2BText(),
    [query, setQuery] = useState(""),
    search = useDebouncedValue(query, 180);
  const products = useRead(
    request,
    "/api/merchant/products?limit=20&search=" + encodeURIComponent(search),
  );
  const rows: { id: string; name: string; productNumber?: string }[] =
    products.data?.elements ?? [];
  const error = (products.error as Error | undefined)?.message ?? "";
  return (
    <section className="b2b-catalog">
      <h3>{t("catalog")}</h3>
      <Field label={t("searchProducts")}>
        <TextInput
          type="search"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
      </Field>
      {error && <Notice tone="danger">{error}</Notice>}
      <div className="b2b-unit-tabs">
        {rows.map((p) => (
          <button
            key={p.id}
            disabled={disabled || (!ids.includes(p.id) && ids.length >= 1000)}
            aria-pressed={ids.includes(p.id)}
            onClick={() =>
              onChange(
                ids.includes(p.id)
                  ? ids.filter((id) => id !== p.id)
                  : [...ids, p.id],
              )
            }
          >
            {p.name} · {p.productNumber ?? p.id}
          </button>
        ))}
      </div>
      <p>
        {t("selectedProducts")} · {ids.length}/1000
      </p>
      <div className="b2b-unit-tabs">
        {ids.map((id) => (
          <button
            key={id}
            disabled={disabled}
            onClick={() => onChange(ids.filter((x) => x !== id))}
          >
            {rows.find((p) => p.id === id)?.name ?? id} ×
          </button>
        ))}
      </div>
    </section>
  );
}
