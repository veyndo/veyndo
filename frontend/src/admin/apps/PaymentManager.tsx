/** Native payment balances and observed financial jobs, shared with the order detail controls. */
import { useRead, useSetRead } from "../shell/useRead";
import { Button, Card, Notice } from "../../kit/ui";
import { useRef, useState } from "react";
import { useAppText } from "../../shared/i18n/app-i18n";
import { useOperationsText } from "../../shared/i18n/operations-i18n";
import type { RequestFn } from "../shell/studio-types";
import { paymentCommand, type PaymentRetry } from "../payments/payment-command";
import PaymentDetails from "../payments/PaymentDetails";
import PaymentActions from "../payments/PaymentActions";
import type { PaymentOptions, Settlement } from "../payments/payment-amount";
import { usePaymentDiagnosticText } from "../../shared/i18n/payment-diagnostic-i18n";
type Data = {
  providers: { id: string; configured: boolean }[];
  attempts: {
    id: string;
    orderId: string;
    state: string;
    currency: string;
    currencyScale: number;
    settlement: Settlement;
  }[];
  jobs: {
    id: string;
    attemptId: string;
    operation: string;
    state: string;
    error?: string;
  }[];
};
export default function PaymentManager({
  request,
  canWrite,
}: {
  request: RequestFn;
  token: string;
  canWrite: boolean;
}) {
  const { a } = useAppText(),
    explain = usePaymentDiagnosticText(),
    { o } = useOperationsText();

  const dataPath = "/api/payments";
  const dataRead = useRead<Data>(request, dataPath);
  const setReadData = useSetRead(request);
  const data = dataRead.data ?? undefined;
  const dataLoadError = (dataRead.error as Error | undefined)?.message;

  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [pending, setPending] = useState<Record<string, string>>({});
  const retries = useRef<Record<string, PaymentRetry>>({});
  const observe = async (id: string, job: string) => {
    for (let attempt = 0; attempt < 15; attempt++) {
      const result = await request(`/api/payments/jobs/${job}`);
      setReadData(dataPath, await request(dataPath));
      if (["succeeded", "failed", "uncertain"].includes(result.state)) {
        setPending((old) => {
          const next = { ...old };
          delete next[id];
          return next;
        });
        if (result.state !== "succeeded")
          throw new Error(explain(result.error) ?? o("providerFailed"));
        delete retries.current[id];
        return true;
      }
      await new Promise((resolve) => setTimeout(resolve, 1000));
    }
    setError(o("providerPending"));
    return false;
  };
  const command = async (
    id: string,
    operation: string,
    amount?: number,
    options?: PaymentOptions,
  ) => {
    setBusy(true);
    setError("");
    try {
      if (pending[id]) return await observe(id, pending[id]);
      const current = paymentCommand(
        retries.current[id],
        id,
        operation,
        amount,
        options,
      );
      retries.current[id] = current;
      const result = await request(`/api/payments/${id}/${operation}`, {
        ...current.body,
        requestKey: current.key,
      });
      setPending((old) => ({ ...old, [id]: result.jobId }));
      return await observe(id, result.jobId);
    } catch (e) {
      setError((e as Error).message);
      return false;
    } finally {
      setBusy(false);
    }
  };
  return (
    <Card className="app-card">
      <h2>{o("payment")}</h2>
      <p>
        {data?.providers.some((p) => p.configured)
          ? a("connected")
          : a("missing")}
      </p>
      {data?.attempts.map((p) => (
        <article className="app-payment" key={p.id}>
          <div className="section-heading">
            <strong>{p.orderId}</strong>
            <span className="operation-status">{o(p.state)}</span>
          </div>
          {
            <PaymentActions
              readOnly={!canWrite}
              state={p.state}
              settlement={p.settlement}
              currency={p.currency}
              scale={p.currencyScale}
              busy={busy}
              pending={
                !!pending[p.id] ||
                !!data.jobs.find(
                  (j) =>
                    j.attemptId === p.id &&
                    ["queued", "running"].includes(j.state),
                )
              }
              onPayment={(op, amount, options) =>
                command(p.id, op, amount, options)
              }
              onCheck={() => void command(p.id, "reconcile")}
            />
          }
          <PaymentDetails id={p.id} request={request} />
        </article>
      ))}
      {(error || dataLoadError) && (
        <Notice tone="danger">{error || dataLoadError}</Notice>
      )}
      <Button
        type="submit"
        disabled={busy}
        onClick={() => void dataRead.refetch()}
      >
        {a("refresh")}
      </Button>
    </Card>
  );
}
