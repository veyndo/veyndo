/** Studio server reads through the query layer, keyed by the request's tenant and path so live and stage data never mix. */
import type { ApiPath } from "../../kit/api";
import { useCallback } from "react";
import { useQuery, useQueryCache, type QueryKey } from "../../kit/query";
import type { RequestFn } from "./studio-types";
import { requestScope } from "./requests";

/** The cache key of one read: tenant, path and (for POST reads) the body. */
type Options = Parameters<RequestFn>[3];
export const readKey = (
  request: RequestFn | undefined,
  path: string,
  body?: unknown,
  options?: Options,
  version?: unknown,
): QueryKey => [
  "read",
  requestScope(request),
  path,
  body === undefined ? null : JSON.stringify(body),
  options === undefined ? null : JSON.stringify(options),
  version === undefined ? null : JSON.stringify(version),
];

/**
 * Reads `path` with the area's request function. A body turns the read into a
 * POST (for read operations exposed as POST). `path` null disables the read.
 */
export function useRead<T = any>(
  request: RequestFn | undefined,
  path: ApiPath | null | undefined,
  {
    body,
    method,
    options,
    version,
    enabled = true,
    staleMs,
    refetchMs,
  }: {
    body?: unknown;
    /** An explicit method for reads exposed as POST; omitted, a body implies POST. */
    method?: string;
    /** Transport options such as the content locale; part of the cache key. */
    options?: Options;
    /** Only part of the cache key: a new value (say a revision) reads again. */
    version?: unknown;
    enabled?: boolean;
    staleMs?: number;
    refetchMs?: number;
  } = {},
) {
  return useQuery<T>(
    readKey(request, path ?? "", body, options, version),
    // Call exactly as a direct read would: no trailing undefined arguments, and
    // a body alone lets the transport choose POST.
    () => {
      const args: Parameters<RequestFn> = [path!, body, method, options];
      while (args.length > 1 && args[args.length - 1] === undefined) args.pop();
      return request!(...args);
    },
    { enabled: enabled && !!path && !!request, staleMs, refetchMs },
  );
}

/** Marks reads of `path` (every body variant) stale so their readers load again. */
export function useInvalidateRead(request: RequestFn) {
  const cache = useQueryCache();
  return useCallback(
    (path: string) => cache.invalidate(["read", requestScope(request), path]),
    [cache, request],
  );
}

/** Replaces the cached answer of a read, for example with a write's response. */
export function useSetRead(request: RequestFn) {
  const cache = useQueryCache();
  return useCallback(
    <T>(path: string, data: T, body?: unknown) =>
      cache.setData(readKey(request, path, body), data),
    [cache, request],
  );
}
