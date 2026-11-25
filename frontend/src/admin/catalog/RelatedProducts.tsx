/** Ordered recommendations retain selected native records across cursor-paginated searches, content-language changes and read-only inspection. */
import { useDebouncedValue } from "../../kit/query";
import { useRead } from "../shell/useRead";
import { Button, Field, TextInput } from "../../kit/ui";
import { mediaImage } from "../../shared/media/product-media";
import { useEffect, useState } from "react";
import type { RequestFn } from "../shell/studio-types";
import { useRelatedProductsText } from "./related-products-i18n";
import RelatedProductRow from "./RelatedProductRow";
import "./related-products.css";
type Choice = {
  id: string;
  name: string;
  productNumber?: string;
  active?: boolean;
  media?: { url: string }[];
};
export default function RelatedProducts({
  request,
  id,
  value,
  onChange,
  language = "en-GB",
  canWrite = true,
}: {
  request: RequestFn;
  id: string;
  value: string[];
  onChange: (ids: string[]) => void;
  language?: string;
  canWrite?: boolean;
}) {
  const t = useRelatedProductsText(),
    [search, setSearch] = useState(""),
    [cursor, setCursor] = useState(""),
    term = useDebouncedValue(search, 200);
  const first = `/api/merchant/products?limit=25&search=${encodeURIComponent(term)}`;
  const page = useRead(
    request,
    cursor ? `${first}&after=${encodeURIComponent(cursor)}` : first,
    { options: { locale: language } },
  );
  // Later pages append to the earlier ones of the same search and language.
  const listKey = JSON.stringify([language, term]);
  const [pages, setPages] = useState<{ key: string; elements: Choice[] }>();
  useEffect(() => {
    if (!page.data) return;
    setPages((old) => ({
      key: listKey,
      elements: [
        ...(cursor && old?.key === listKey ? old.elements : []),
        ...(page.data.elements ?? []),
      ],
    }));
  }, [page.data]);
  const current =
    search === term
      ? {
          elements: pages?.key === listKey ? pages.elements : [],
          next: page.data?.nextCursor ?? undefined,
          error: (page.error as Error | undefined)?.message,
          loading: page.status === "idle" || page.status === "loading",
        }
      : undefined;
  const selected = [...new Set(value)],
    products = current?.elements ?? [];
  const move = (index: number, offset: number) => {
    const next = [...selected];
    [next[index], next[index + offset]] = [next[index + offset], next[index]];
    onChange(next);
  };
  return (
    <div className="related-products-workspace">
      <p className="related-products-hint">{t("hint")}</p>
      <section aria-label={t("selected")}>
        <div className="related-products-heading">
          <h3>{t("selected")}</h3>
          <span>{t("count", { count: selected.length })}</span>
        </div>
        {selected.length ? (
          <ol className="related-products-selected">
            {selected.map((productId, index) => (
              <RelatedProductRow
                key={productId}
                id={productId}
                request={request}
                language={language}
                index={index}
                total={selected.length}
                disabled={!canWrite}
                onMove={(offset) => move(index, offset)}
                onRemove={() =>
                  onChange(selected.filter((v) => v !== productId))
                }
              />
            ))}
          </ol>
        ) : (
          <p className="related-products-hint">{t("empty")}</p>
        )}
      </section>
      <section aria-label={t("search")}>
        <Field label={t("search")}>
          <TextInput
            type="search"
            value={search}
            onChange={(e) => {
              setSearch(e.target.value);
              setCursor("");
            }}
          />
        </Field>
        {selected.length >= 20 && (
          <p className="related-products-hint">{t("limit")}</p>
        )}
        <div
          className="related-products-results"
          aria-busy={!current || current.loading}
        >
          {products
            .filter((p) => p.id !== id)
            .map((p) => (
              <label key={p.id} className="related-product-choice">
                <input
                  type="checkbox"
                  checked={selected.includes(p.id)}
                  disabled={
                    !canWrite ||
                    (!selected.includes(p.id) && selected.length >= 20)
                  }
                  onChange={(e) =>
                    onChange(
                      e.target.checked
                        ? [...selected, p.id]
                        : selected.filter((v) => v !== p.id),
                    )
                  }
                />
                {mediaImage(p.media) && (
                  <img
                    src={mediaImage(p.media)}
                    alt=""
                    width="36"
                    height="36"
                  />
                )}
                <span>
                  <strong>{p.name}</strong>
                  <small>
                    {p.productNumber ?? p.id}
                    {p.active === false ? ` · ${t("inactive")}` : ""}
                  </small>
                </span>
              </label>
            ))}
        </div>
        {(!current || current.loading) && (
          <p role="status" className="related-products-hint">
            {t("loading")}
          </p>
        )}
        {current?.error && (
          <div role="alert">
            <p>{current.error}</p>
            <Button type="button" onClick={() => void page.refetch()}>
              {t("retry")}
            </Button>
          </div>
        )}
        {current &&
          !current.loading &&
          !current.error &&
          !products.some((p) => p.id !== id) && (
            <p className="related-products-hint">{t("noMatches")}</p>
          )}
        {current?.next && (
          <Button
            type="button"
            disabled={current.loading}
            onClick={() => setCursor(current.next!)}
          >
            {t("more")}
          </Button>
        )}
      </section>
    </div>
  );
}
