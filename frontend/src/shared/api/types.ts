/** Request contracts: the open one app surfaces receive and the catalogue-checked one first-party screens use. */
import type { ApiPath } from "../../kit/api";

type RequestOptions = {
  idempotencyKey?: string;
  salesChannel?: string;
  locale?: string;
};
/**
 * The request function app surfaces receive. App routes are discovered per shop
 * at run time, so the path is open; the transport still enforces the shop scope.
 */
export type RequestFn = (
  path: string,
  body?: unknown,
  method?: string,
  options?: RequestOptions,
) => Promise<any>;
/**
 * The same transport for Studio and storefront screens: the path must be one the
 * generated route catalogue (`kit/api`) lists, so a path the server does not
 * mount does not compile.
 */
export type ApiRequestFn = (
  path: ApiPath,
  body?: unknown,
  method?: string,
  options?: RequestOptions,
) => Promise<any>;
/** Hands a screen's transport to an app surface, whose routes the catalogue cannot list. */
export const forApps = (request: ApiRequestFn): RequestFn =>
  request as RequestFn;
