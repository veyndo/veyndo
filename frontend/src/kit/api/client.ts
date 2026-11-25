/** The generated API client: typed route calls over the route catalogue and typed native operation calls over MCP. */
import type {
  OperationInputs,
  OperationName,
  OperationOutputs,
} from "./generated/operations";
import type { JsonValue } from "./generated/operations/value";
import {
  fillPath,
  type PathParams,
  type RouteKey,
  type RoutePathOf,
} from "./routes";

/**
 * Sends one request and resolves with the parsed JSON body; rejects on HTTP errors.
 * Studio's request function and the storefront's session request both fit.
 */
export type Transport = (
  path: string,
  body?: unknown,
  method?: string,
) => Promise<any>;

/** The declared result of an operation, or open JSON when it declares none. */
export type OperationResult<N extends OperationName> =
  N extends keyof OperationOutputs ? OperationOutputs[N] : JsonValue;

/** A native operation refused or failed; `operation` names it. */
export class OperationError extends Error {
  constructor(
    message: string,
    readonly operation: string,
    readonly code?: number,
  ) {
    super(message);
    this.name = "OperationError";
  }
}

type RouteOptions<K extends RouteKey> = {
  query?: Record<string, string | number | boolean | undefined>;
  body?: unknown;
} & (keyof PathParams<RoutePathOf<K>> extends never
  ? { params?: undefined }
  : { params: PathParams<RoutePathOf<K>> });

type RouteArgs<K extends RouteKey> = keyof PathParams<
  RoutePathOf<K>
> extends never
  ? [options?: RouteOptions<K>]
  : [options: RouteOptions<K>];

let sequence = 0;

export function createApiClient(transport: Transport) {
  return {
    /** Calls a catalogued route; the key and its path parameters are checked at compile time. Invalid parameters reject. */
    async route<K extends RouteKey>(
      key: K,
      ...[options]: RouteArgs<K>
    ): Promise<unknown> {
      const space = key.indexOf(" ");
      const method = key.slice(0, space);
      let path = fillPath(
        key.slice(space + 1),
        (options?.params ?? {}) as Record<string, string | number>,
      );
      const query = Object.entries(options?.query ?? {}).filter(
        ([, value]) => value !== undefined,
      );
      if (query.length)
        path +=
          "?" +
          new URLSearchParams(
            query.map(([name, value]) => [name, String(value)]),
          ).toString();
      return transport(path, options?.body, method);
    },
    /** Calls a native operation through the hosted MCP endpoint and returns its structured result. */
    async call<N extends OperationName>(
      name: N,
      args: OperationInputs[N],
    ): Promise<OperationResult<N>> {
      const response = await transport(
        "/mcp",
        {
          jsonrpc: "2.0",
          id: `op-${++sequence}`,
          method: "tools/call",
          params: { name, arguments: args },
        },
        "POST",
      );
      if (response?.error)
        throw new OperationError(
          String(response.error.message ?? "Operation failed"),
          name,
          response.error.code,
        );
      const result = response?.result;
      if (!result || result.isError)
        throw new OperationError(
          String(result?.content?.[0]?.text ?? "Operation failed"),
          name,
        );
      return (result.structuredContent ?? null) as OperationResult<N>;
    },
  };
}

export type ApiClient = ReturnType<typeof createApiClient>;
