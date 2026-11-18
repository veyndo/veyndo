/** Generated native operation input contracts; do not edit. */
/** Native input DTOs; opaque JSON fields remain explicit. */
import type { JsonValue } from "./value";
export interface Inputs10 {
  "knowledge.autonomy.revert": { approve?: boolean; productId?: string };
  "knowledge.autonomy.status": JsonValue;
  "knowledge.claims": { productId?: string };
  "knowledge.claim.propose": {
    confidence?: number;
    contentHash?: string;
    locale?: string;
    nodeType?:
      | "product"
      | "variant"
      | "material"
      | "property"
      | "intent"
      | "problem"
      | "occasion"
      | "audience"
      | "claim"
      | "return_reason"
      | "supplier"
      | "policy"
      | "document"
      | "support";
    productId?: string;
    quote?: string;
    sourceId?: string;
    text?: string;
  };
  "knowledge.claim.review": {
    approve?: boolean;
    id?: string;
    revision?: number;
    state?: "evidenced" | "confirmed" | "rejected";
  };
  "knowledge.extract": {
    inference?: JsonValue;
    locale?: string;
    productId?: string;
    sourceId?: string;
  };
  "knowledge.compile": {
    claims?: Array<{ id: string; text: string }>;
    productId?: string;
  };
  "checkout.payment.methods": JsonValue;
  "checkout.payment.method.revoke": {
    approve: boolean;
    id: string;
    requestKey: string;
  };
  "checkout.payment.instructions": { id: string };
  "merchant.payment.instructions": { id: string };
  "merchant.payment": {
    amountMinor?: number;
    approve: boolean;
    captureId?: string;
    finalCapture?: boolean;
    id: string;
    operation:
      "capture" | "authorize" | "void" | "refund" | "reconcile" | "cancel";
    requestKey: string;
  };
}
