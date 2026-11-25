/** Capture timing uses the installed provider declaration and the existing versioned settings draft. */
import { useRead } from "../../../../../frontend/src/admin/shell/useRead";
import { Field, Select } from "../../../../../frontend/src/kit/ui";
import type { Payment } from "../../../../../frontend/src/shared/api/shop-api";
import type { RequestFn } from "../../../../../frontend/src/admin/shell/studio-types";
import { usePaymentLedgerText } from "../../../../../frontend/src/shared/i18n/payment-ledger-i18n";
export default function CaptureModeSettings({
  payment,
  request,
  onChange,
}: {
  payment: Payment;
  request: RequestFn;
  onChange: (value: Pick<Payment, "captureMode">) => void;
}) {
  const text = usePaymentLedgerText();
  const providers = useRead(request, "/api/payment-providers").data;
  const method = providers?.providers
    ?.find((p: any) => p.id === payment.provider)
    ?.contract?.methods?.find((m: any) => m.id === payment.providerMethod);
  const capabilities: string[] = method?.capabilities ?? [];
  if (!capabilities.includes("authorize") && !payment.captureMode) return null;
  return (
    <Field label={text("timing")}>
      <Select
        value={payment.captureMode ?? ""}
        onChange={(e) =>
          onChange({
            captureMode: (e.target.value ||
              undefined) as Payment["captureMode"],
          })
        }
      >
        <option value="">{text("inherit")}</option>
        <option value="capture">{text("immediate")}</option>
        <option value="authorize">{text("deferred")}</option>
      </Select>
    </Field>
  );
}
