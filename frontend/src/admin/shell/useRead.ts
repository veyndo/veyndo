/** Studio server reads through the query layer, keyed by the request's tenant and path so live and stage data never mix. */
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
): QueryKey => [
  "read",
  requestScope(request),
  path,
  body === undefined ? null : JSON.stringify(body),
  options === undefined ? null : JSON.stringify(options),
];

/**
 * Reads `path` with the area's request function. A body turns the read into a
 * POST (for read operations exposed as POST). `path` null disables the read.
 */
export function useRead<T = any>(
  request: RequestFn | undefined,
  path: string | null | undefined,
  {
    body,
    options,
    enabled = true,
    staleMs,
    refetchMs,
  }: {
    body?: unknown;
    /** Transport options such as the content locale; part of the cache key. */
    options?: Options;
    enabled?: boolean;
    staleMs?: number;
    refetchMs?: number;
  } = {},
) {
  return useQuery<T>(
    readKey(request, path ?? "", body, options),
    // A body makes the transport send POST, exactly as a direct call would.
    () =>
      options !== undefined
        ? request!(path!, body, undefined, options)
        : body === undefined
          ? request!(path!)
          : request!(path!, body),
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
