/** Current source passages for the order workspace. The query contains no order or customer data. */
import { useRead } from "../shell/useRead";
import { Button, Notice } from "../../kit/ui";
import type { RequestFn } from "../shell/studio-types";
import type { PreviewResult } from "../intelligence/knowledge-types";
import { useCompositionText } from "./composition-i18n";

export default function OrderShippingPolicy({
  request,
}: {
  request: RequestFn;
}) {
  const t = useCompositionText();
  const query = t("shippingQuery");
  const preview = useRead<PreviewResult>(request, "/api/knowledge/preview", {
    body: { audience: "merchant", query, includeExternal: false },
  });
  const latest = preview.fetching
    ? undefined
    : preview.status === "error"
      ? { sources: undefined }
      : preview.data && { sources: preview.data.sources };
  const sources = latest?.sources;
  return (
    <section className="agent-shipping-policy" aria-label={t("shippingPolicy")}>
      <h3>{t("shippingPolicy")}</h3>
      {!latest ? (
        <p role="status">{t("shippingLoading")}</p>
      ) : !sources ? (
        <>
          <Notice tone="danger">{t("shippingError")}</Notice>
          <Button type="button" onClick={() => void preview.refetch()}>
            {t("shippingRetry")}
          </Button>
        </>
      ) : sources.length === 0 ? (
        <p>{t("shippingEmpty")}</p>
      ) : (
        <ul>
          {sources.slice(0, 3).map((source) => (
            <li key={source.sourceId}>
              <strong lang={source.locale}>{source.title}</strong>
              <p lang={source.locale}>{source.text}</p>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
