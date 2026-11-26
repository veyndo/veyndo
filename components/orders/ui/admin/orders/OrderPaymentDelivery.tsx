/** Payment jobs are observed until confirmation. Delivery actions share the server state machine. */
import StudioSlot from "../../../../../frontend/src/admin/shell/registry/StudioSlot";
import { safeRichUrl } from "../../../../../frontend/src/shared/content/rich-document";
import { useAccountText } from "../../../../../frontend/src/shared/i18n/account-i18n";
import { useState } from "react";
import { useOperationsText } from "../../../../../frontend/src/shared/i18n/operations-i18n";
import ShipmentSplit, { type ShipmentPosition } from "./ShipmentSplit";
import { useShipmentText } from "../../../../../frontend/src/shared/i18n/shipment-i18n";
import "./shipments.css";
import "./orders.css";
import {
  Badge,
  Button,
  Card,
  Field,
  TextInput,
} from "../../../../../frontend/src/kit/ui";
import type { OrderAction } from "./OrderWorkflow";
export default function OrderPaymentDelivery({
  order,
  busy,
  rights,
  onAction,
  onChanged,
  onSplit,
}: {
  order: any;
  busy: boolean;
  rights: string[];
  onAction: (a: OrderAction, tracking?: string, trackingUrl?: string) => void;
  /** Reloads the order, for example after a payment command settled. */
  onChanged: () => Promise<unknown>;
  onSplit?: (index: number, positions: ShipmentPosition[]) => Promise<boolean>;
}) {
  const { o } = useOperationsText();
  const shipmentText = useShipmentText();
  const { a: accountText } = useAccountText();
  const [links, setLinks] = useState<Record<number, string>>({});
  const [tracking, setTracking] = useState<Record<number, string>>({});
  const actions = order.workflow?.actions ?? [];
  const label = (a: OrderAction) =>
    o(
      a.kind === "payment"
        ? "markPaid"
        : a.state === "shipped"
          ? "markShipped"
          : "markDelivered",
    );
  return (
    <Card
      level={2}
      title={o("payment")}
      actions={<Badge>{o(order.payment.state)}</Badge>}
    >
      <p>{o(order.payment.provider)}</p>
      {rights.includes("orders.write") &&
        actions
          .filter((a: OrderAction) => a.kind === "payment")
          .map((a: OrderAction) => (
            <Button
              key={a.id}
              tone="primary"
              disabled={busy || !a.enabled}
              onClick={() => onAction(a)}
            >
              {label(a)}
            </Button>
          ))}
      <StudioSlot
        location="admin.order.payment"
        context={{
          payment: order.payment,
          currency: order.cart.price.currency ?? "EUR",
          scale: order.cart.price.currencyScale ?? 2,
          onChanged,
        }}
      />
      <div className="orders-divider" />
      <h2>{o("delivery")}</h2>
      {order.deliveries.length ? (
        order.deliveries.map((d: any, index: number) => (
          <div className="orders-package" key={index}>
            <div className="section-heading">
              <strong>
                {shipmentText("package")} {index + 1} ·{" "}
                {o(d.shippingMethod.name)}
              </strong>
              <Badge>{o(d.state)}</Badge>
            </div>
            <ul className="shipment-positions">
              {(d.positions ?? []).map((p: ShipmentPosition) => (
                <li key={p.id}>
                  <span>
                    {order.lineItems?.find(
                      (item: any) =>
                        item.referencedId === p.id || item.id === p.id,
                    )?.label ?? p.id}
                  </span>
                  <strong>× {p.quantity}</strong>
                </li>
              ))}
            </ul>
            {onSplit &&
              rights.includes("orders.write") &&
              rights.includes("customers.pii") && (
                <ShipmentSplit
                  delivery={d}
                  busy={busy}
                  enabled={
                    !!order.workflow?.shipmentSplits?.find(
                      (s: any) => s.deliveryIndex === index,
                    )?.enabled
                  }
                  labels={Object.fromEntries(
                    (order.lineItems ?? []).map((item: any) => [
                      item.referencedId ?? item.id,
                      item.label,
                    ]),
                  )}
                  onSplit={(positions) => onSplit(index, positions)}
                />
              )}
            {d.trackingCode && <p>{d.trackingCode}</p>}
            {d.trackingUrl?.startsWith("https://") &&
              safeRichUrl(d.trackingUrl) && (
                <a
                  href={d.trackingUrl}
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  {accountText("trackShipment")}
                </a>
              )}
            {rights.includes("orders.write") &&
              actions
                .filter(
                  (a: OrderAction) =>
                    a.kind === "delivery" && a.deliveryIndex === index,
                )
                .map((a: OrderAction) => (
                  <div key={a.id}>
                    {a.state === "shipped" && (
                      <>
                        <Field label={o("tracking")}>
                          <TextInput
                            value={tracking[index] ?? ""}
                            onChange={(e) =>
                              setTracking({
                                ...tracking,
                                [index]: e.target.value,
                              })
                            }
                            maxLength={100}
                          />
                        </Field>
                        <Field label={accountText("trackingUrl")}>
                          <TextInput
                            type="url"
                            maxLength={2000}
                            value={links[index] ?? ""}
                            onChange={(e) =>
                              setLinks({ ...links, [index]: e.target.value })
                            }
                          />
                        </Field>
                      </>
                    )}
                    <Button
                      tone="primary"
                      disabled={busy || !a.enabled}
                      onClick={() =>
                        onAction(
                          a,
                          tracking[index] ?? "",
                          links[index] ?? d.trackingUrl ?? "",
                        )
                      }
                    >
                      {label(a)}
                    </Button>
                    {a.reason && <small>{o(a.reason)}</small>}
                  </div>
                ))}
          </div>
        ))
      ) : (
        <p>{o("digital")}</p>
      )}
    </Card>
  );
}
