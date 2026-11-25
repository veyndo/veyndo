/** Bounded, debounced native product/SKU lookup; prices are only committed through the reviewed checkout quote. */
import { useDebouncedValue } from "../../kit/query";
import { useRead } from "../shell/useRead";
import { Field, Notice, TextInput } from "../../kit/ui";
import { mediaImage } from "../../shared/media/product-media";
import { useState } from "react";
import type { RequestFn } from "../../shared/api/types";
import type { Product } from "../../shared/api/shop-api";
import { useAdminCreationText } from "../../shared/i18n/admin-creation-i18n";
export default function OrderProductPicker({
  request,
  onAdd,
  disabled,
}: {
  request: RequestFn;
  onAdd: (product: Product) => void;
  disabled: boolean;
}) {
  const { n } = useAdminCreationText();
  const [query, setQuery] = useState(""),
    term = useDebouncedValue(query, 250);
  const search = useRead(
    request,
    `/api/merchant/products?active=true&limit=20&search=${encodeURIComponent(term)}`,
  );
  const loading = query !== term || search.status === "idle" || search.fetching;
  const error = (search.error as Error | undefined)?.message ?? "";
  const products: Product[] = search.error ? [] : (search.data?.elements ?? []);
  return (
    <div>
      <Field label={n("productSearch")}>
        <TextInput
          type="search"
          maxLength={200}
          disabled={disabled}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
      </Field>
      {error && <Notice tone="danger">{error}</Notice>}
      <div className="orders-product-results">
        {products.map((p) => (
          <button
            type="button"
            key={p.id}
            disabled={disabled || p.stock < 1}
            onClick={() => onAdd(p)}
          >
            {mediaImage(p.media) && <img src={mediaImage(p.media)} alt="" />}
            <span>
              <strong>{p.name}</strong>
              <small>
                {p.product_number ?? p.id}{" "}
                {Object.values(p.options ?? {}).join(" · ")}
              </small>
            </span>
            {n("add")}
          </button>
        ))}
      </div>
      <small role="status">
        {loading ? n("loading") : products.length ? null : n("noProducts")}
      </small>
    </div>
  );
}
