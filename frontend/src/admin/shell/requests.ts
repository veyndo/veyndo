/** Authenticated Studio transport; staging changes only the tenant, never the principal. */
import { requestJson } from "../../shared/api/request-json";
import { useMemo, useRef } from "react";
import { responseError } from "../../shared/i18n/errors-i18n";
import type { RequestFn } from "./studio-types";

/** The tenant each Studio request function sends to; cached reads key by it. */
export const requestScopes = new WeakMap<RequestFn, string>();
let anonymous = 0;
/**
 * The tenant of a request function. An unregistered function (a wrapper or a test
 * double) gets an identity of its own, so its reads are never shared with another.
 */
export function requestScope(request: RequestFn | undefined) {
  if (typeof request !== "function") return "";
  let scope = requestScopes.get(request);
  if (scope === undefined) {
    scope = `function:${++anonymous}`;
    requestScopes.set(request, scope);
  }
  return scope;
}

export function createStudioRequest(
  token: string,
  tenant: string,
  locale: string,
): RequestFn {
  const send: RequestFn = async (path, body, method, options) => {
    const multipart = body instanceof FormData;
    const response = await requestJson(path, {
      method: method ?? (body === undefined ? "GET" : "POST"),
      headers: {
        ...(multipart ? {} : { "Content-Type": "application/json" }),
        Authorization: `Bearer ${token}`,
        "x-tenant": tenant,
        "x-commerce-locale": options?.locale ?? locale,
        ...(options?.salesChannel
          ? { "sw-sales-channel-id": options.salesChannel }
          : {}),
        ...(options?.idempotencyKey
          ? { "Idempotency-Key": options.idempotencyKey }
          : {}),
      },
      body:
        body === undefined
          ? undefined
          : multipart
            ? body
            : JSON.stringify(body),
    });
    const value = response.value;
    if (!response.ok)
      throw responseError(
        value.errors?.[0]?.detail || response.statusText,
        response.status,
        value.errors?.[0],
      );
    return value;
  };
  requestScopes.set(send, tenant);
  return send;
}
export function useStudioRequests(
  token: string,
  workspace: string,
  environment: string,
  locale: string,
) {
  const credential = useRef(token);
  credential.current = token;
  return useMemo(() => {
    const scoped = (tenant: string): RequestFn => {
      const send: RequestFn = (path, body, method, options) =>
        createStudioRequest(credential.current, tenant, locale)(
          path,
          body,
          method,
          options,
        );
      requestScopes.set(send, tenant);
      return send;
    };
    const liveRequest = scoped(workspace);
    return {
      liveRequest,
      request: environment ? scoped(environment) : liveRequest,
    };
  }, [workspace, environment, locale]);
}
