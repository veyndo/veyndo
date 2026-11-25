/** Product-scoped review publication; authoritative authorization stays in the API. */
import {
  useRead,
  useSetRead,
} from "../../../../../frontend/src/admin/shell/useRead";
import { Button, Card, Notice } from "../../../../../frontend/src/kit/ui";
import { useState } from "react";
import type { ApiRequestFn as RequestFn } from "../../../../../frontend/src/shared/api/types";
import Icon from "../../../../../frontend/src/shared/ui/Icon";
import { useProductEditorText } from "./product-editor-i18n";
import { useShopText } from "../../../../../frontend/src/shared/i18n/shop-i18n";
type Review = {
  id: string;
  productId: string;
  author: string;
  title: string;
  content: string;
  rating: number;
  approved: boolean;
  sourceImport?: {
    source: string;
    sourceId: string;
    sourceShop: string;
  } | null;
};
export default function ReviewModeration({
  request,
  productId,
}: {
  request: RequestFn;
  productId: string;
}) {
  const { s } = useShopText();
  const e = useProductEditorText();
  const commerce = useRead(request, "/api/merchant/commerce");
  const setRead = useSetRead(request);
  const loading = commerce.status === "idle" || commerce.status === "loading";
  const reviews: Review[] = (commerce.data?.reviews ?? []).filter(
    (r: Review) => r.productId === productId,
  );
  const [actionError, setError] = useState("");
  const error =
    actionError || (commerce.error as Error | undefined)?.message || "";
  const [busy, setBusy] = useState(false);
  return (
    <Card>
      <h2>{s("moderation")}</h2>
      <p>{s("reviewHint")}</p>
      {error && <Notice tone="danger">{error}</Notice>}
      {!loading && !error && reviews.length === 0 && (
        <div className="product-review-empty">
          <Icon name="chat" />
          <strong>{e("noReviews")}</strong>
          <p>{e("reviewEmpty")}</p>
        </div>
      )}
      {reviews.map((r) => (
        <article className="commerce-review" key={r.id}>
          <div>
            <strong>{r.title}</strong>
            <small>
              {r.author} · {r.rating}/5
            </small>
            <p>{r.content}</p>
            {r.sourceImport && (
              <>
                <small>{s("importedReview")}</small>
                <details>
                  <summary>{s("reviewSource")}</summary>
                  <p>
                    {r.sourceImport.source} · {r.sourceImport.sourceShop} ·{" "}
                    {r.sourceImport.sourceId}
                  </p>
                </details>
              </>
            )}
          </div>
          <Button
            type="submit"
            disabled={busy}
            onClick={async () => {
              setBusy(true);
              setError("");
              try {
                await request(
                  `/api/merchant/reviews/${r.id}`,
                  { approved: !r.approved },
                  "PUT",
                );
                setRead("/api/merchant/commerce", {
                  ...commerce.data,
                  reviews: commerce.data.reviews.map((v: Review) =>
                    v.id === r.id ? { ...v, approved: !r.approved } : v,
                  ),
                });
              } catch (e) {
                setError((e as Error).message);
              } finally {
                setBusy(false);
              }
            }}
          >
            {s(r.approved ? "hide" : "approve")}
          </Button>
        </article>
      ))}
    </Card>
  );
}
