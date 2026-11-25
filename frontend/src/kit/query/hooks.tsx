/** React bindings of the query cache: a provider per scope, useQuery for reads and useMutation for writes that invalidate. */
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import { keyText, QueryCache, type QueryKey, type QueryState } from "./cache";

const CacheContext = createContext<QueryCache | null>(null);

/**
 * One cache per `scope` (for example the shop and stage). Changing the scope drops
 * the old cache, so data never crosses a tenant or sandbox boundary.
 */
export function QueryProvider({
  scope,
  children,
}: {
  scope: string;
  children: ReactNode;
}) {
  const cache = useMemo(() => new QueryCache(), [scope]);
  useEffect(() => () => cache.dispose(), [cache]);
  return (
    <CacheContext.Provider value={cache}>{children}</CacheContext.Provider>
  );
}

// Outside a provider (isolated views, tests) every hook shares this one cache.
// Studio always renders inside a QueryProvider scoped per shop, stage and person.
let fallback = new QueryCache();

/** Drops the fallback cache; test setups call it between tests. */
export function resetFallbackQueryCache() {
  fallback.dispose();
  fallback = new QueryCache();
}

/** The provider's cache, or the shared fallback outside a provider. */
export function useQueryCache() {
  return useContext(CacheContext) ?? fallback;
}

export type QueryResult<T> = QueryState<T> & {
  /** Loads again now, ignoring freshness. */
  refetch: () => Promise<T | undefined>;
};

/**
 * Reads `key` through `load`. Equal keys share data and in-flight loads. Mounting
 * reloads data older than `staleMs` (default: always) while still showing it; an
 * invalidation reloads at once. A disabled query keeps its last state.
 */
export function useQuery<T>(
  key: QueryKey,
  load: () => Promise<T>,
  {
    enabled = true,
    staleMs = 0,
    refetchMs,
  }: {
    enabled?: boolean;
    staleMs?: number;
    /** Reload this often while mounted and the page is visible (polling). */
    refetchMs?: number;
  } = {},
): QueryResult<T> {
  const cache = useQueryCache();
  const text = keyText(key);
  const loader = useRef(load);
  loader.current = load;
  // The key is compared by value; a new array with the same entries is the same key.
  const stableKey = useMemo(() => key, [text]);
  const state = useSyncExternalStore(
    useCallback(
      (notify) => cache.subscribe(stableKey, notify),
      [cache, stableKey],
    ),
    () => cache.state<T>(stableKey),
  );
  // Load on mount, on a key change and whenever the entry is invalidated; failed
  // loads wait for an explicit refetch or invalidation instead of looping.
  const needsLoad =
    state.updatedAt === undefined &&
    !state.fetching &&
    state.status !== "error";
  useEffect(() => {
    if (enabled)
      cache.fetch(stableKey, () => loader.current(), staleMs).catch(() => {});
  }, [cache, stableKey, enabled, staleMs, needsLoad]);
  useEffect(() => {
    if (!enabled || !refetchMs) return;
    const timer = setInterval(() => {
      if (document.visibilityState !== "hidden") cache.invalidate(stableKey);
    }, refetchMs);
    return () => clearInterval(timer);
  }, [cache, stableKey, enabled, refetchMs]);
  const refetch = useCallback(async () => {
    cache.invalidate(stableKey);
    try {
      return await cache.fetch(stableKey, () => loader.current());
    } catch {
      return undefined;
    }
  }, [cache, stableKey]);
  return { ...state, refetch };
}

/**
 * Runs a write; on success it invalidates the given key prefixes so every
 * reader of them reloads. The result and error of the last run are kept.
 */
export function useMutation<I, O>(
  write: (input: I) => Promise<O>,
  { invalidates = [] }: { invalidates?: QueryKey[] } = {},
) {
  const cache = useQueryCache();
  const [state, setState] = useState<{
    status: "idle" | "running" | "success" | "error";
    data?: O;
    error?: unknown;
  }>({ status: "idle" });
  const writer = useRef(write);
  writer.current = write;
  const prefixes = useRef(invalidates);
  prefixes.current = invalidates;
  const run = useCallback(
    async (input: I) => {
      setState((previous) => ({ ...previous, status: "running" }));
      try {
        const data = await writer.current(input);
        prefixes.current.forEach((prefix) => cache.invalidate(prefix));
        setState({ status: "success", data });
        return data;
      } catch (error) {
        setState({ status: "error", error });
        throw error;
      }
    },
    [cache],
  );
  return { ...state, run };
}
