/** Generated native operation input contracts; do not edit. */
/** Native input DTOs; opaque JSON fields remain explicit. */
import type { JsonValue } from "./value";
export interface Inputs8 {
  "merchant.scenario.status": { id: string };
  "merchant.scenario.list": JsonValue;
  "merchant.scenario.readiness": JsonValue;
  "merchant.scenario.stock": { approve: boolean; quantity: number };
  "merchant.scenario.attacks": JsonValue;
  "merchant.scenario.probe": JsonValue;
  "merchant.apps.search": { after?: string; limit?: number; query?: string };
  "merchant.apps.read": { action: string; app: string; arguments: JsonValue };
  "merchant.agents.status": JsonValue;
  "merchant.agents.control": {
    approve: true;
    revision: number;
    stopped: boolean;
  };
  "merchant.grid.plan": {
    entity: "orders" | "products" | "customers";
    instruction: string;
    locale: string;
  };
  "merchant.customer.lookup": {
    after?: string;
    limit?: number;
    query?: string;
  };
}
