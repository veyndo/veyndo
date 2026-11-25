/** Order workspace: server actions, exact-once commands, provider progress and visible event history. */
import { forApps } from "../../shared/api/types";
import { paymentCommand } from "../payments/payment-command";
import { usePaymentDiagnosticText } from "../../shared/i18n/payment-diagnostic-i18n";
import type { PaymentOptions } from "../payments/payment-amount";

import { contentText } from "../../shared/i18n/content-language";
import CoreExtensions from "../../shared/apps/CoreExtensions";
import { useCrmText } from "../../shared/i18n/crm-i18n";
import EntityHistory from "../../shared/history/EntityHistory";
import AddressCard from "../../shared/customer/AddressCard";
import { useCustomerText } from "../../shared/i18n/customer-i18n";
import "../../shared/styles/customers.css";

import { useCallback, useEffect, useRef, useState } from "react";
import { useOperationsText } from "../../shared/i18n/operations-i18n";
import type { RequestFn } from "../shell/studio-types";
import OrderPaymentDelivery from "./OrderPaymentDelivery";
import OrderWorkflow, { type OrderAction } from "./OrderWorkflow";
import { useShipmentText } from "../../shared/i18n/shipment-i18n";
import type { ShipmentPosition } from "./ShipmentSplit";
import ReceiptPanel from "./ReceiptPanel";
import ConfigurationSummary from "../../shared/apps/ConfigurationSummary";
import { configurationSnapshots } from "../../shared/api/configuration-types";
import { useUnsavedWorkspace } from "../../shared/ui/useUnsavedWorkspace";
import {
  Badge,
  Button,
  Card,
  Field,
  Notice,
  Spinner,
  TextArea,
} from "../../kit/ui";
import "./orders.css";
export default function OrderDetail({
  id,
  request,
  rights,
  onBack,
  download,
  onCustomer,
  onProduct,
  onDirtyChange,
}: {
  onDirtyChange?: (dirty: boolean) => void;
  onCustomer?: (email: string) => void;
  onProduct?: (id: string) => void;
  id: string;
  request: RequestFn;
  rights: string[];
  onBack: () => void;
  download: (path: string) => Promise<void>;
}) {
  const { o, locale } = useOperationsText();
  const explainPayment = usePaymentDiagnosticText();
  const { r } = useCrmText();
  const shipmentText = useShipmentText();
  const { c } = useCustomerText();
  const [order, setOrder] = useState<any>(),
    [error, setError] = useState(""),
    [busy, setBusy] = useState(false),
    [feedback, setFeedback] = useState(""),
    [note, setNote] = useState(""),
    [pendingJob, setPendingJob] = useState<string>();
  const inflight = useRef(false);
  useUnsavedWorkspace(!!note.trim(), onDirtyChange);
  const retry = useRef<
    { signature: string; key: string; body: any } | undefined
  >(undefined);
  const load = useCallback(async () => {
    const next = await request(`/api/merchant/orders/${id}`);
    setOrder(next);
    return next;
  }, [request, id]);
  useEffect(() => {
    void load().catch((e) => setError(e.message));
  }, [load]);
  const run = async (fn: () => Promise<boolean | void>) => {
    if (inflight.current) return false;
    inflight.current = true;
    setBusy(true);
    setError("");
    setFeedback(o("processing"));
    try {
      const done = await fn();
      await load();
      if (done !== false) setFeedback(o("applied"));
      return done !== false;
    } catch (e) {
      setError((e as Error).message);
      if ((e as Error & { status?: number }).status === 409)
        retry.current = undefined;
      setFeedback("");
      await load().catch(() => {});
      return false;
    } finally {
      inflight.current = false;
      setBusy(false);
    }
  };
  const transition = (a: OrderAction, tracking = "", trackingUrl = "") =>
    void run(async () => {
      const signature = JSON.stringify({
        id,
        action: a.id,
        tracking,
        trackingUrl,
      });
      if (retry.current?.signature !== signature)
        retry.current = {
          signature,
          key: crypto.randomUUID(),
          body: {
            revision: order.revision,
            kind: a.kind,
            state: a.state,
            action: a.kind === "order" ? a.id : undefined,
            deliveryIndex: a.deliveryIndex,
            trackingCode: tracking,
            trackingUrl: a.kind === "delivery" ? trackingUrl : undefined,
          },
        };
      const current = retry.current;
      const result = await request(`/api/merchant/orders/${id}/transition`, {
        ...current.body,
        requestKey: current.key,
      });
      setOrder({ ...order, ...result });
      retry.current = undefined;
    });
  const observe = async (job: string) => {
    setPendingJob(job);
    setFeedback(o("providerWorking"));
    for (let i = 0; i < 60; i++) {
      const result = await request(`/api/payments/jobs/${job}`);
      if (result.state === "succeeded") {
        setPendingJob(undefined);
        retry.current = undefined;
        return true;
      }
      if (["failed", "uncertain"].includes(result.state)) {
        setPendingJob(undefined);
        throw new Error(explainPayment(result.error) ?? o("providerFailed"));
      }
      await new Promise((resolve) => setTimeout(resolve, 1000));
    }
    setFeedback(o("providerPending"));
    return false;
  };
  const payment = (
    operation: string,
    amount?: number,
    options?: PaymentOptions,
  ) =>
    run(async () => {
      if (pendingJob) return observe(pendingJob);
      const current = paymentCommand(
        retry.current,
        order.payment.attemptId,
        operation,
        amount,
        options,
      );
      retry.current = current;
      const result = await request(
        `/api/payments/${order.payment.attemptId}/${operation}`,
        { ...current.body, requestKey: current.key },
      );
      return observe(result.jobId);
    });
  const money = (n: number) =>
    new Intl.NumberFormat(locale, {
      style: "currency",
      currency: order?.cart.price.currency ?? "EUR",
    }).format(n);
  if (!order) return <p role="status">{error || o("processing")}</p>;
  return (
    <>
      <div className="orders-toolbar">
        <Button onClick={onBack}>← {r("back")}</Button>
        <Button
          disabled={busy}
          onClick={() => void run(() => load().then(() => true))}
        >
          {o("refresh")}
        </Button>
      </div>
      <CoreExtensions
        objectId={id}
        extensions={order.extensions}
        omittedExtensions={order.omittedExtensions}
        request={forApps(request)}
      />
      <div className="orders-heading">
        <div>
          <small className="orders-eyebrow">{o("order")}</small>
          <h2>{order.orderNumber}</h2>
          <p>
            {order.customerEmail &&
            (order.orderCustomer?.customerId ||
              order.orderCustomer?.guest === true) &&
            onCustomer &&
            rights.includes("customers.read") ? (
              <button
                type="button"
                className="entity-link"
                onClick={() => onCustomer(order.customerEmail)}
              >
                {order.customerEmail}
              </button>
            ) : (
              (order.customerEmail ?? "—")
            )}{" "}
            · {new Date(order.createdAt).toLocaleString(locale)}
          </p>
        </div>
        <div className="orders-total">
          <small className="orders-eyebrow">{o("orderTotal")}</small>
          <strong>{money(order.cart.price.totalPrice)}</strong>
        </div>
      </div>
      {error && <Notice tone="danger">{error}</Notice>}
      {feedback && (
        <Notice tone={busy ? "accent" : "positive"}>
          {busy ? (
            <>
              <Spinner decorative /> {feedback}
            </>
          ) : (
            feedback
          )}
        </Notice>
      )}
      <OrderWorkflow
        order={order}
        busy={busy}
        canEdit={rights.includes("orders.write")}
        onAction={transition}
      />
      <div className="orders-detail-grid">
        <Card
          level={2}
          title={o("items")}
          actions={<Badge>{order.cart.lineItems.length}</Badge>}
        >
          <div className="operations-table">
            {order.cart.lineItems.map((item: any) => (
              <div className="operation-row" key={item.id}>
                <div>
                  {onProduct &&
                  rights.includes("catalog.read") &&
                  (item.type == null || item.type === "product") &&
                  typeof item.referencedId === "string" ? (
                    <button
                      type="button"
                      className="entity-link"
                      onClick={() => onProduct(item.referencedId)}
                    >
                      <strong>{item.label}</strong>
                    </button>
                  ) : (
                    <strong>{item.label}</strong>
                  )}
                  <small>{item.referencedId}</small>
                  <ConfigurationSummary
                    configurations={configurationSnapshots(item)}
                    language={locale}
                  />
                </div>
                <span>× {item.quantity}</span>
                <strong>{money(item.price.totalPrice)}</strong>
              </div>
            ))}
          </div>
          <div className="customer-address-grid orders-addresses">
            <div>
              <small className="orders-eyebrow">{c("billingAddress")}</small>
              <AddressCard
                address={
                  order.billingAddress ?? order.cart.checkout?.billingAddress
                }
              />
            </div>
            <div>
              <small className="orders-eyebrow">{c("shippingAddress")}</small>
              <AddressCard
                address={order.shippingAddress ?? order.cart.checkout?.address}
              />
            </div>
          </div>
          <small>{c("snapshotHint")}</small>
        </Card>
        <OrderPaymentDelivery
          request={request}
          order={order}
          busy={busy}
          rights={rights}
          onAction={transition}
          onSplit={(index: number, positions: ShipmentPosition[]) =>
            run(async () => {
              const signature = JSON.stringify({
                operation: "delivery.split",
                id,
                index,
                positions,
              });
              if (retry.current?.signature !== signature)
                retry.current = {
                  signature,
                  key: crypto.randomUUID(),
                  body: {
                    revision: order.revision,
                    deliveryIndex: index,
                    positions,
                  },
                };
              const current = retry.current;
              await request(`/api/merchant/orders/${id}/deliveries/split`, {
                ...current.body,
                requestKey: current.key,
              });
              retry.current = undefined;
            })
          }
          onPayment={payment}
          pendingJob={!!pendingJob}
          onCheck={() => void run(() => observe(pendingJob!))}
        />
      </div>
      {rights.includes("documents.read") && (
        <ReceiptPanel
          order={order}
          request={request}
          canCreate={rights.includes("documents.create")}
          download={download}
        />
      )}
      <EntityHistory
        request={request}
        entity="order"
        id={id}
        revision={order.revision}
      />
      <Card level={2} title={o("activity")}>
        {rights.includes("orders.write") && (
          <form
            className="orders-note"
            onSubmit={(e) => {
              e.preventDefault();
              void run(async () => {
                await request(`/api/merchant/orders/${id}/notes`, {
                  revision: order.revision,
                  text: note,
                });
                setNote("");
              });
            }}
          >
            <Field label={o("note")}>
              <TextArea
                value={note}
                onChange={(e) => setNote(e.target.value)}
                required
                maxLength={4000}
              />
            </Field>
            <Button type="submit" tone="primary" disabled={busy}>
              {o("addNote")}
            </Button>
          </form>
        )}
        <div className="orders-timeline">
          {order.activity.map((event: any) => (
            <div className="orders-event" key={event.id}>
              <span className="orders-event-dot" aria-hidden="true" />
              <div>
                <small>
                  {new Date(event.createdAt).toLocaleString(locale)} ·{" "}
                  {event.actor}
                </small>
                <p>
                  {["note", "flow"].includes(event.kind)
                    ? event.data.text
                    : event.kind === "delivery.split"
                      ? shipmentText("splitEvent")
                      : event.kind === "receipt"
                        ? `${o(event.data.kind)} ${event.data.number}`
                        : `${o(event.data.kind)} → ${contentText(order.workflow.states.find((s: any) => s.id === event.data.state)?.label ?? {}, locale, order.workflow.mainLocale ?? "en-GB") || o(event.data.state)}`}
                </p>
                {event.kind === "flow" && (
                  <Badge>
                    {o("flow")} · {event.data.event}
                  </Badge>
                )}
              </div>
            </div>
          ))}
        </div>
      </Card>
    </>
  );
}
