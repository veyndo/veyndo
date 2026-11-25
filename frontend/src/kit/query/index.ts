/** Public interface of kit/query: the scoped in-memory query cache and its React hooks. */
export { QueryCache, keyText } from "./cache";
export type { QueryKey, QueryState } from "./cache";
export { QueryProvider, useQuery, useMutation, useQueryCache } from "./hooks";
export type { QueryResult } from "./hooks";
