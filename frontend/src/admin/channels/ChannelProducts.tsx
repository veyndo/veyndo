/** Search-based channel product assignment, preserving selected IDs across server-filtered result pages. */
import { useDebouncedValue } from "../../kit/query";
import { useRead } from "../shell/useRead";
import { Button, Field, Notice, TextInput } from "../../kit/ui";
import { useEffect, useState } from "react";
import type { RequestFn } from "../shell/studio-types";
import { useChannelText } from "./channel-i18n";
export default function ChannelProducts({
  request,
  value,
  onChange,
}: {
  request: RequestFn;
  value: string[];
  onChange: (value: string[]) => void;
}) {
  const t = useChannelText(),
    [search, setSearch] = useState(""),
    [names, setNames] = useState<Record<string, string>>({}),
    term = useDebouncedValue(search, 200);
  const products = useRead(
    request,
    `/api/merchant/products?limit=25&search=${encodeURIComponent(term)}`,
  );
  const items: any[] = products.data?.elements ?? [];
  const error = (products.error as Error | undefined)?.message ?? "";
  // Names of every product seen stay known for the chosen list.
  useEffect(() => {
    if (!products.data) return;
    setNames((old) => ({
      ...old,
      ...Object.fromEntries(
        products.data.elements.map((p: any) => [p.id, p.name]),
      ),
    }));
  }, [products.data]);
  return (
    <div>
      <Field label={t("search")}>
        <TextInput
          type="search"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
      </Field>
      <div className="channel-product-chips">
        {value.map((id) => (
          <Button
            type="button"
            key={id}
            aria-label={`${t("remove")} ${names[id] ?? id}`}
            onClick={() => onChange(value.filter((v) => v !== id))}
          >
            {names[id] ?? id} ×
          </Button>
        ))}
      </div>
      <div className="channel-product-results">
        {items
          .filter((p) => !value.includes(p.id))
          .map((p) => (
            <button
              type="button"
              className="search-hit"
              key={p.id}
              disabled={value.length >= 500}
              onClick={() => onChange([...value, p.id])}
            >
              <strong>{p.name}</strong>
              <small>
                {p.productNumber} · {t("add")} +
              </small>
            </button>
          ))}
      </div>
      {error && <Notice tone="danger">{error}</Notice>}
    </div>
  );
}
