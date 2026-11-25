# kit/api

The generated API client. Both contracts it builds on are generated from Rust and
drift-checked by the structure gate:

- `generated/routes.ts`: every static route the Rust router mounts, keyed
  `"METHOD /path"`, from `scripts/api_catalogue.py`.
- `generated/operations/`: input and declared output types of every native
  operation, from the compiled registry by `scripts/testing/operation_contracts.py`.

## Modules

- [`index.ts`](index.ts): The public interface; import only this file from outside `kit/`.
- [`client.ts`](client.ts): `createApiClient(transport)` with `route(key, {params, query, body})` and `call(operation, arguments)`; operation failures throw `OperationError`.
- [`routes.ts`](routes.ts): The catalogue as records, path-parameter types and `fillPath`, which refuses parameters that could escape their segment.
- [`generated/`](generated/): Generated sources; do not edit.

The client grants nothing: the server checks shop, identity and current rights for
every route and operation. Request and response bodies of plain routes are not typed
yet; operations carry their declared types.

## Verification

Run `npm test -- kit-api` from `frontend/`, and `python3 scripts/api_catalogue.py --check`
and `python3 scripts/testing/operation_contracts.py --check` from the repository root.
