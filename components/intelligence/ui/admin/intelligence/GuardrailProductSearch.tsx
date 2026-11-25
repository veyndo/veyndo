/** Bounded native catalogue search, not the sampled dashboard; stale replies never replace current results. */
import { useDebouncedValue } from "../../../../../frontend/src/kit/query";
import { useRead } from "../../../../../frontend/src/admin/shell/useRead";
import { Field, Notice, TextInput } from "../../../../../frontend/src/kit/ui";
import { mediaImage } from "../../../../../frontend/src/shared/media/product-media";
import { useState } from "react";
import type {
  Product,
  RequestFn,
} from "../../../../../frontend/src/admin/shell/studio-types";
import { useKnowledgeText } from "../../../../../frontend/src/shared/i18n/knowledge-i18n";
import Icon from "../../../../../frontend/src/shared/ui/Icon";
export default function GuardrailProductSearch({
  request,
  products,
  selected,
  disabled,
  onAdd,
}: {
  request: RequestFn;
  products: Product[];
  selected: string[];
  disabled: boolean;
  onAdd: (p: Product) => void;
}) {
  const k = useKnowledgeText();
  const [query, setQuery] = useState(""),
    term = useDebouncedValue(query, 250);
  const search = useRead(
    request,
    `/api/merchant/products?limit=20&search=${encodeURIComponent(term)}`,
  );
  const settled = query === term && !search.fetching;
  const error = settled
    ? ((search.error as Error | undefined)?.message ?? "")
    : "";
  // The given products show until the first search answers.
  const results: Product[] = !settled
    ? query
      ? []
      : products
    : search.error
      ? []
      : (search.data?.elements ?? products);
  const busy = !!query && !settled;
  return (
    <div className="guardrail-search">
      <Field label={k("policySearch")}>
        <TextInput
          type="search"
          maxLength={200}
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
          }}
        />
      </Field>
      {error && <Notice tone="danger">{error}</Notice>}
      <div className="guardrail-search-results">
        {results.map((p) => (
          <button
            className="guardrail-search-product"
            type="button"
            aria-label={`${p.name} · ${k(selected.includes(p.id) ? "policyAdded" : "policyAdd")}`}
            key={p.id}
            disabled={disabled || busy || selected.includes(p.id)}
            onClick={() => onAdd(p)}
          >
            {mediaImage(p.media) ? (
              <img src={mediaImage(p.media)} alt="" />
            ) : (
              <span className="guardrail-product-icon">
                <Icon name="box" />
              </span>
            )}
            <span>
              <strong>{p.name}</strong>
              <small>
                {selected.includes(p.id) ? k("policyAdded") : k("policyAdd")}
              </small>
            </span>
            <Icon name={selected.includes(p.id) ? "check" : "plus"} />
          </button>
        ))}
      </div>
      <small role="status">
        {busy
          ? k("loading")
          : !results.length && !error
            ? k("policyNoResults")
            : null}
      </small>
    </div>
  );
}
