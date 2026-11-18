/** Generated native operation input contracts; do not edit. */
/** Native input DTOs; opaque JSON fields remain explicit. */
import type { JsonValue } from "./value";
export interface Inputs17 {
  "automation.compare_history": {
    condition: JsonValue;
    flowId: string;
    limit: number;
    revision: number;
    ruleId: string;
  };
  "automation.validate_flow": { data: JsonValue; id: string; revision: number };
}
