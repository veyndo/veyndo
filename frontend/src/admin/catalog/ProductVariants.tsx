/** Native variant family browser with cursor pagination, explicit editing and a bounded creation wizard. */
import type { ApiPath } from "../../kit/api";
import { useInvalidateRead, useRead } from "../shell/useRead";
import { Button, Notice } from "../../kit/ui";
import { useEffect, useState } from "react";
import type { RequestFn } from "../shell/studio-types";
import { useCatalogText } from "../../shared/i18n/catalog-i18n";
import { type ProductDraft } from "./catalog-model";
import PairFields from "./PairFields";
import VariantGenerator from "./VariantGenerator";
import Icon from "../../shared/ui/Icon";
import { useVariantText } from "./variant-i18n";
import "./variants.css";
export default function ProductVariants({
  draft,
  request,
  onOpen,
  onChange,
  dirty = false,
  canWrite = true,
}: {
  canWrite?: boolean;
  draft: ProductDraft;
  request: RequestFn;
  onOpen: (id: string) => void;
  onChange: (draft: ProductDraft) => void;
  dirty?: boolean;
}) {
  const { c, money } = useCatalogText(),
    v = useVariantText();
  const firstPath: ApiPath = `/api/merchant/products?limit=50&parentId=${encodeURIComponent(draft.id!)}`;
  const first = useRead(request, draft.catalog.parentId ? null : firstPath);
  const invalidate = useInvalidateRead(request);
  // Later pages append to the first page until it is loaded again.
  const [more, setMore] = useState<{ items: any[]; cursor: string | null }>();
  useEffect(() => setMore(undefined), [first.data]);
  const items: any[] = [
    ...(first.data?.elements ?? []),
    ...(more?.items ?? []),
  ];
  const cursor: string | null =
    more?.cursor !== undefined ? more.cursor : (first.data?.nextCursor ?? null);
  const [actionError, setError] = useState(""),
    [paging, setBusy] = useState(false);
  const busy = paging || first.status === "loading";
  const error =
    actionError || (first.error as Error | undefined)?.message || "";
  if (draft.catalog.parentId)
    return (
      <>
        <Button
          type="button"
          disabled={dirty}
          onClick={() => onOpen(draft.catalog.parentId!)}
        >
          {v("parent")}
        </Button>
        <h3>{c("options")}</h3>
        <fieldset className="catalog-permission-fields" disabled={!canWrite}>
          <PairFields
            value={draft.catalog.options}
            onChange={(options) =>
              onChange({ ...draft, catalog: { ...draft.catalog, options } })
            }
          />
        </fieldset>
      </>
    );
  return (
    <div className="product-variants">
      <p className="variant-family-hint">{c("variantHint")}</p>
      {dirty && <p role="status">{c("unsaved")}</p>}
      {busy && <p role="status">{v("loading")}</p>}
      {!busy && !error && !items.length && (
        <div className="variant-empty">
          <Icon name="layers" size={28} />
          <h3>{v("empty")}</h3>
          <p>{v("emptyHint")}</p>
        </div>
      )}
      {!!items.length && (
        <h3 className="variant-family-title">
          {v("family")}{" "}
          <span>
            ({items.length}
            {cursor ? "+" : ""})
          </span>
        </h3>
      )}
      {items.map((p) => (
        <button
          type="button"
          className="catalog-variant"
          key={p.id}
          disabled={dirty || busy}
          onClick={() => onOpen(p.id)}
        >
          <span className="variant-identity">
            <strong>
              {Object.entries(p.options)
                .map(([k, value]) => `${k}: ${value}`)
                .join(" · ")}
            </strong>
            <span>
              {p.productNumber} · {money(p.price)} · {p.stock} {c("stock")}
            </span>
          </span>
          <span className="variant-open">
            {v("edit")} <Icon name="arrow" size={16} />
          </span>
        </button>
      ))}
      {cursor && (
        <Button
          type="button"
          disabled={busy}
          onClick={async () => {
            setBusy(true);
            setError("");
            try {
              const result = await request(
                `/api/merchant/products?limit=50&parentId=${encodeURIComponent(draft.id!)}&after=${encodeURIComponent(cursor)}`,
              );
              setMore((old) => ({
                items: [...(old?.items ?? []), ...result.elements],
                cursor: result.nextCursor,
              }));
            } catch (e) {
              setError((e as Error).message);
            } finally {
              setBusy(false);
            }
          }}
        >
          {v("more")}
        </Button>
      )}
      {error && <Notice tone="danger">{error}</Notice>}
      {canWrite && (
        <details className="variant-create">
          <summary>
            <Icon name="plus" size={18} />
            {v("start")}
          </summary>
          <fieldset
            className="catalog-permission-fields"
            disabled={dirty || busy}
          >
            <VariantGenerator
              parent={draft}
              request={request}
              onCreated={async () => invalidate(firstPath)}
            />
          </fieldset>
        </details>
      )}
    </div>
  );
}
