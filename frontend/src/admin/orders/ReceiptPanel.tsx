/** Seller configuration and version-bound receipt creation/download; reads go through the query layer. */
import { useCustomerText } from "../../shared/i18n/customer-i18n";

import { useState } from "react";
import { useShipmentText } from "../../shared/i18n/shipment-i18n";
import { useOperationsText } from "../../shared/i18n/operations-i18n";
import type { RequestFn } from "../shell/studio-types";
import { useMutation, useQuery } from "../../kit/query";
import { Button, Card, Field, Notice, Select } from "../../kit/ui";
export default function ReceiptPanel({
  order,
  request,
  canCreate,
  download,
}: {
  order: any;
  request: RequestFn;
  canCreate: boolean;
  download: (p: string) => Promise<void>;
}) {
  const { o, locale } = useOperationsText();
  const shipmentText = useShipmentText();
  const [deliveryIndex, setDeliveryIndex] = useState(0);
  const { c } = useCustomerText();
  const [kind, setKind] = useState("invoice"),
    [reference, setReference] = useState(""),
    [downloadError, setDownloadError] = useState(""),
    [downloading, setDownloading] = useState(false);
  const settings = useQuery(["receipts", "settings"], () =>
    request("/api/merchant/receipts/settings"),
  );
  const listing = useQuery(["orders", order.id, "receipts"], () =>
    request(`/api/merchant/orders/${order.id}/receipts`),
  );
  const create = useMutation(
    () =>
      request(`/api/merchant/orders/${order.id}/receipts`, {
        revision: order.revision,
        kind,
        locale: locale.slice(0, 2),
        requestKey: crypto.randomUUID(),
        ...(kind === "delivery_note" && order.deliveries.length
          ? { deliveryIndex }
          : {}),
        ...(kind === "cancellation" ? { referenceId: reference } : {}),
      }),
    { invalidates: [["orders", order.id, "receipts"]] },
  );
  const ready = !!settings.data?.data?.name;
  const receipts: any[] = listing.data?.elements ?? [];
  const busy = create.status === "running" || downloading;
  const error =
    downloadError ||
    (create.status === "error" && (create.error as Error).message) ||
    (settings.error as Error | undefined)?.message ||
    (listing.error as Error | undefined)?.message;
  return (
    <Card level={2} title={o("documents")}>
      <p>{o("documentHint")}</p>
      {error && <Notice tone="danger">{error}</Notice>}
      {canCreate && (
        <>
          <p>{c("sellerSettings")}</p>
          <form
            className="commerce-fields"
            onSubmit={(e) => {
              e.preventDefault();
              setDownloadError("");
              void create.run(undefined).catch(() => {});
            }}
          >
            <Field label={o("documents")}>
              <Select value={kind} onChange={(e) => setKind(e.target.value)}>
                {["invoice", "delivery_note", "cancellation"].map((k) => (
                  <option key={k} value={k}>
                    {o(k)}
                  </option>
                ))}
              </Select>
            </Field>
            {kind === "delivery_note" && order.deliveries.length > 0 && (
              <Field label={shipmentText("package")}>
                <Select
                  value={deliveryIndex}
                  onChange={(e) => setDeliveryIndex(Number(e.target.value))}
                >
                  {order.deliveries.map((_: unknown, index: number) => (
                    <option key={index} value={index}>
                      {shipmentText("package")} {index + 1}
                    </option>
                  ))}
                </Select>
              </Field>
            )}
            {kind === "cancellation" && (
              <Field label={o("reference")}>
                <Select
                  required
                  value={reference}
                  onChange={(e) => setReference(e.target.value)}
                >
                  <option value="">—</option>
                  {receipts
                    .filter((r) => r.kind === "invoice")
                    .map((r) => (
                      <option key={r.id} value={r.id}>
                        {r.number}
                      </option>
                    ))}
                </Select>
              </Field>
            )}
            <Button type="submit" tone="primary" disabled={busy || !ready}>
              {o("generate")}
            </Button>
          </form>
        </>
      )}
      {receipts.map((r) => (
        <div className="operation-row" key={r.id}>
          <strong>{r.number}</strong>
          <span>
            {o(r.kind)} · {r.locale.toUpperCase()}
          </span>
          <Button
            onClick={async () => {
              setDownloading(true);
              setDownloadError("");
              try {
                await download(r.pdfPath);
              } catch (e) {
                setDownloadError((e as Error).message);
              } finally {
                setDownloading(false);
              }
              void listing.refetch();
            }}
          >
            {o("pdf")}
          </Button>
        </div>
      ))}
    </Card>
  );
}
