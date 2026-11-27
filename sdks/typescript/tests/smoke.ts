/** Smoke test of the generated TypeScript SDK against a running Veyndo server (BASE_URL); synthetic accounts only. Run by scripts/sdk_smoke.py with Node type stripping. */
import { Auth, McpProtocolError, VeyndoClient, VeyndoError, paginate, type JsonValue, type ProblemKind } from "../src/index.ts";

declare const process: { env: Record<string, string | undefined> };

const base = process.env.BASE_URL;
if (!base) throw new Error("BASE_URL is required");
const shop = process.env.SDK_SMOKE_SHOP ?? "atelier";

// Responses are open JSON; the smoke test reads the fields it asserts.
type Row = { [key: string]: any };
const row = (value: JsonValue): Row => value as Row;

function check(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message);
}

async function expectError(call: () => Promise<unknown>, status: number, kind: ProblemKind): Promise<VeyndoError> {
  try {
    await call();
  } catch (error) {
    check(error instanceof VeyndoError, `expected VeyndoError, got ${String(error)}`);
    check(error.status === status && error.kind === kind, `expected ${status} ${kind}, got ${error.status} ${error.kind}`);
    check(error.problems[0]?.detail, "problem without detail");
    return error;
  }
  throw new Error(`expected HTTP ${status} ${kind}`);
}

const anonymous = new VeyndoClient({ baseUrl: base });
check(row(await anonymous.host.getHealth()).status === "ok", "health");
await expectError(() => anonymous.identity.getAuthSession(), 401, "authentication_required");

const store = anonymous.with({ tenant: shop });
const complete = row(await store.catalog.postStoreProduct({ body: { limit: 100 } })).elements as Row[];
const paged: Row[] = [];
for await (const product of paginate((after) => store.catalog.postStoreProduct({ body: { limit: 2, after } }))) {
  paged.push(row(product));
}
check(paged.length > 2 && paged.map((p) => p.id).join() === complete.map((p) => p.id).join(), "pagination returns the complete list in order");
await expectError(() => store.catalog.postStoreProduct({ body: { limit: 0 } }), 400, "validation_error");

const found = row(await store.mcp.call("catalog.search", { limit: 2 }));
check(found.elements.length === 2, "typed catalog.search");
const invalid = await expectError(() => store.mcp.call("catalog.search", { limit: 0 }), 400, "validation_error");
check(invalid.problems[0]?.pointer === "/limit", "tool validation pointer");
await expectError(() => store.mcp.call("merchant.products", {}), 403, "permission_denied");
try {
  await store.mcp.rpc("tools/call", { name: "no.such.tool", arguments: {} });
  throw new Error("an unknown tool must be a protocol error");
} catch (error) {
  check(error instanceof McpProtocolError && error.code === -32602, `expected MCP -32602, got ${String(error)}`);
}

const suffix = crypto.randomUUID().slice(0, 12);
const account = row(
  await anonymous.identity.postAuthRegister({
    body: { email: `sdk-ts-${suffix}@example.test`, name: "SDK smoke", password: "Synthetic-sdk-2026!", workspaceId: `sdk-ts-${suffix}` },
  }),
);
const personal = anonymous.with({ auth: Auth.personalSession(account.token), tenant: account.workspace });
check(row(await personal.identity.getAuthSession()).user.email === `sdk-ts-${suffix}@example.test`, "personal session");
const key = row(
  await personal.identity.postWorkspaceIntegrations({ body: { name: "sdk-smoke", permissions: ["catalog.read"], expiresInDays: 1 } }),
);
const integration = anonymous.with({ auth: Auth.integrationKey(key.key), tenant: account.workspace });
check((await integration.mcp.listTools()).some((tool) => tool.name === "merchant.products"), "integration key sees merchant tools");
check(Array.isArray(row(await integration.mcp.call("merchant.products", { limit: 5 })).elements), "typed merchant.products");
await expectError(() => integration.identity.getWorkspaceIntegrations(), 403, "permission_denied");
console.log("PASS TypeScript SDK: health, store pagination, typed HTTP/MCP failures, personal session, integration key and MCP tools");
