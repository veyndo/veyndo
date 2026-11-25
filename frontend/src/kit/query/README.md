# kit/query

The query layer for server data. Reads go through `useQuery(key, load)`; equal keys
share one cached result and one in-flight load, fresh data (`staleMs`) is not loaded
again, and `useMutation(write, {invalidates})` makes every reader of the given key
prefixes reload after a successful write.

The cache lives in memory only and belongs to a `QueryProvider` scope: Studio keys it
by shop, stage and person, so changing any of them drops all cached data. Outside a
provider each component gets a private cache, so isolated views work without sharing. Loads that started before
an invalidation or a scope change never overwrite newer state. Nothing is written to
browser storage; credentials stay with the transport the loader uses.

## Modules

- [`index.ts`](index.ts): The public interface.
- [`cache.ts`](cache.ts): `QueryCache` with fetch, setData, prefix invalidation and disposal.
- [`hooks.tsx`](hooks.tsx): `QueryProvider`, `useQuery`, `useMutation`, `useQueryCache`.

## Verification

Run `npm test -- kit-query` from `frontend/`.
