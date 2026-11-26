/** Public interface of kit/api: the generated route catalogue, operation contracts and the typed client over them. */
export { createApiClient, OperationError } from "./client";
export type { ApiClient, OperationResult, Transport } from "./client";
export { routes, fillPath, childPath, withQuery } from "./routes";
export type {
  ApiPath,
  ApiRoutePath,
  Route,
  RouteKey,
  RoutePathOf,
  PathParams,
} from "./routes";
export type {
  OperationCall,
  OperationInputs,
  OperationName,
  OperationOutputs,
} from "./generated/operations";
export type { JsonValue } from "./generated/operations/value";
