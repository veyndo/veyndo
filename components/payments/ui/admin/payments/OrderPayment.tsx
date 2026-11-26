/** One order's payment ledger in Studio: provider commands with exact-once keys, job progress and details; contributed to the order page. */
import { useRef, useState } from "react";
import { Notice, Spinner } from "../../../../../frontend/src/kit/ui";
import { useStudio } from "../../../../../frontend/src/admin/shell/StudioContext";
import { useOperationsText } from "../../../../../frontend/src/shared/i18n/operations-i18n";
import { usePaymentDiagnosticText } from "../../../../../frontend/src/shared/i18n/payment-diagnostic-i18n";
import PaymentActions from "./PaymentActions";
import PaymentDetails from "./PaymentDetails";
import { paymentCommand, type PaymentRetry } from "./payment-command";
import type { OrderPaymentContext, PaymentOptions } from "./payment-amount";

export default function OrderPayment({
  context: { payment, currency, scale, onChanged },
}: {
  context: OrderPaymentContext;
}) {
  const { request, access } = useStudio();
  const { o } = useOperationsText();
  const explain = usePaymentDiagnosticText();
  const [busy, setBusy] = useState(false),
    [pendingJob, setPendingJob] = useState<string>(),
    [error, setError] = useState(""),
    [feedback, setFeedback] = useState("");
  const retry = useRef<PaymentRetry | undefined>(undefined);
  const inflight = useRef(false);
  const attempt = payment.attemptId;
  if (!attempt) return null;
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
        throw new Error(explain(result.error) ?? o("providerFailed"));
      }
      await new Promise((resolve) => setTimeout(resolve, 1000));
    }
    setFeedback(o("providerPending"));
    return false;
  };
  const run = async (command: () => Promise<boolean>) => {
    if (inflight.current) return false;
    inflight.current = true;
    setBusy(true);
    setError("");
    setFeedback(o("processing"));
    try {
      const done = await command();
      await onChanged();
      if (done) setFeedback(o("applied"));
      return done;
    } catch (e) {
      setError((e as Error).message);
      if ((e as Error & { status?: number }).status === 409)
        retry.current = undefined;
      setFeedback("");
      await onChanged().catch(() => {});
      return false;
    } finally {
      inflight.current = false;
      setBusy(false);
    }
  };
  const command = (
    operation: string,
    amount?: number,
    options?: PaymentOptions,
  ) =>
    run(async () => {
      if (pendingJob) return observe(pendingJob);
      const current = paymentCommand(
        retry.current,
        attempt,
        operation,
        amount,
        options,
      );
      retry.current = current;
      const result = await request(`/api/payments/${attempt}/${operation}`, {
        ...current.body,
        requestKey: current.key,
      });
      return observe(result.jobId);
    });
  return (
    <>
      {access.includes("payments.manage") && (
        <PaymentActions
          state={payment.state}
          settlement={payment.capturedMinor === undefined ? undefined : payment}
          currency={currency}
          scale={scale}
          busy={busy}
          pending={!!pendingJob}
          onPayment={command}
          onCheck={() => void run(() => observe(pendingJob!))}
        />
      )}
      {access.includes("payments.read") && (
        <PaymentDetails key={attempt} id={attempt} request={request} />
      )}
      {error && <Notice tone="danger">{error}</Notice>}
      {feedback && (
        <p role="status">
          {busy && <Spinner decorative />} {feedback}
        </p>
      )}
    </>
  );
}
