/** PreviewPanel renders verified shop state and typed user actions. */
import { useDebouncedValue } from "../../kit/query";
import { useRead } from "../shell/useRead";
import { Button, Field, Notice, Select } from "../../kit/ui";
import { mediaImage } from "../../shared/media/product-media";
import { useEffect, useState } from "react";
import { useLocale } from "../../shared/i18n/i18n";
import { inheritedText } from "../../shared/geography/geography-types";
import type { CustomerGroup } from "../../shared/customer/customer-types";
import Icon from "../../shared/ui/Icon";
import { useWorkspaceText } from "../../shared/i18n/workspace-i18n";
import type { Product, RequestFn } from "../shell/studio-types";
export function PreviewPanel({
  product,
  request,
  connected,
  onIntent,
}: {
  product: Product | undefined;
  request: RequestFn;
  connected: boolean;
  onIntent: (s: string) => void;
}) {
  const { w } = useWorkspaceText();
  const { t, money, locale } = useLocale();
  const [quantity, setQuantity] = useState(1);
  const [group, setGroup] = useState("consumer");
  const groupRead = useRead(
    request,
    connected ? "/api/merchant/customer-groups" : null,
  );
  const groups: CustomerGroup[] = groupRead.data?.elements ?? [];
  const mainLocale: string = groupRead.data?.mainLocale ?? "en-GB";
  useEffect(() => {
    if (groupRead.data) setGroup("consumer");
  }, [groupRead.data]);
  useEffect(() => {
    setQuantity(product?.min_purchase || 1);
  }, [product?.id]);
  // The quote follows product, quantity and group 200 ms after they settle.
  const wanted =
    product && connected
      ? { productId: product.id, quantity, customerGroup: group }
      : undefined;
  const settled = useDebouncedValue(wanted, 200);
  const current = JSON.stringify(wanted) === JSON.stringify(settled);
  const quoteRead = useRead(request, settled ? "/api/merchant/quote" : null, {
    body: settled,
    version: [product?.revision, locale],
  });
  const loading = !!wanted && (!current || quoteRead.fetching);
  const quote = current && !quoteRead.fetching ? quoteRead.data : undefined;
  const error = current && quoteRead.status === "error";
  const line = quote?.quote.lineItems[0];
  return (
    <aside className="studio-preview">
      <div className="preview-heading">
        <Icon name="box" />
        <span>{t("livePreview")}</span>
      </div>
      {product ? (
        <>
          <div className="preview-art">
            {mediaImage(product.media) ? (
              <img src={mediaImage(product.media)} alt={product.name} />
            ) : (
              <div className="preview-empty">
                <span aria-hidden="true">◇</span>
                <p>{w("noImages")}</p>
              </div>
            )}
          </div>
          <div className="preview-product">
            <span className="kicker">{product.id}</span>
            <h2>{product.name}</h2>
            <p>{product.description}</p>
            <div className="product-price">
              <strong>{money(product.price)}</strong>
              <span>
                {product.stock} · {t("stock")}
              </span>
            </div>
          </div>
          <section className="price-playground">
            <div className="card-heading">
              <h3>{t("simulation")}</h3>
              <Icon name="pulse" size={18} />
            </div>
            <p>{t("simulationHint")}</p>
            <Field label={t("group")}>
              <Select value={group} onChange={(e) => setGroup(e.target.value)}>
                {groups.map((g) => (
                  <option key={g.id} value={g.id}>
                    {inheritedText(
                      g.translations,
                      locale,
                      mainLocale,
                      "name",
                    ) || g.id}
                  </option>
                ))}
              </Select>
            </Field>
            <label>
              {t("quantity")}
              <div className="quantity-input">
                <button
                  aria-label="−"
                  disabled={quantity <= 1}
                  onClick={() => setQuantity((q) => Math.max(1, q - 1))}
                >
                  −
                </button>
                <input
                  aria-label={t("quantity")}
                  type="number"
                  min="1"
                  max="10000"
                  value={quantity}
                  onChange={(e) =>
                    setQuantity(
                      Math.max(1, Math.min(10000, Number(e.target.value) || 1)),
                    )
                  }
                />
                <button
                  aria-label="+"
                  disabled={quantity >= 10000}
                  onClick={() => setQuantity((q) => Math.min(10000, q + 1))}
                >
                  +
                </button>
              </div>
            </label>
            <small>
              {t("quantitiesHint", {
                min: product.min_purchase,
                steps: product.purchase_steps,
              })}
            </small>
            <div
              className="quote-result"
              aria-live="polite"
              aria-busy={loading}
            >
              {error ? (
                <Notice tone="danger">{t("failure")}</Notice>
              ) : line ? (
                <>
                  <div>
                    <span>{t("effectiveQuantity")}</span>
                    <b>{quote.effectiveQuantity}</b>
                  </div>
                  <div>
                    <span>{t("unitPrice")}</span>
                    <b>{money(line.price.unitPrice)}</b>
                  </div>
                  <div>
                    <span>
                      {quote.quote.price.taxStatus === "net"
                        ? t("net")
                        : t("gross")}
                    </span>
                    <strong>{money(quote.quote.price.positionPrice)}</strong>
                  </div>
                  <div className="quote-total">
                    <span>{t("total")}</span>
                    <strong>{money(quote.quote.price.totalPrice)}</strong>
                  </div>
                </>
              ) : (
                <p>{t("thinking")}</p>
              )}
            </div>
            <Button
              type="submit"
              disabled={!connected}
              onClick={() =>
                onIntent(t("promptSim", { product: product.name }))
              }
            >
              {t("useKnowledge")}
              <Icon name="arrow" size={16} />
            </Button>
          </section>
        </>
      ) : (
        <div className="preview-empty">
          <Icon name="graph" size={48} />
          <p>{connected ? w("previewNoProduct") : t("connectFirst")}</p>
        </div>
      )}
    </aside>
  );
}
