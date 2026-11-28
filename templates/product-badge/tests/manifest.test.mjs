/** Offline checks every app template's Manifest passes before the server compiles it: identity, languages, references and access. */
// Identical in every app template (templates/shared/tests/manifest.test.mjs; scripts/templates.py checks it).
// The server's native compiler remains the authority; `npm run deploy` reports its exact errors.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { deploy, dryRun } from "../scripts/deploy-sandbox.mjs";

const manifest = JSON.parse(await readFile(new URL("../manifest.json", import.meta.url), "utf8"));
const LANGUAGES = ["en", "de", "es"];

function localized(value, path, out = []) {
  if (Array.isArray(value)) value.forEach((item, i) => localized(item, `${path}/${i}`, out));
  else if (value && typeof value === "object") {
    if (typeof value.en === "string") out.push([path, value]);
    else for (const [key, item] of Object.entries(value)) if (key !== "inputSchema") localized(item, `${path}/${key}`, out);
  }
  return out;
}

test("identity is a stable declarative app", () => {
  assert.match(manifest.id, /^[a-z][a-z0-9_]{2,40}$/);
  assert.match(manifest.version, /^\d+\.\d+\.\d+$/);
  assert.equal(manifest.coreApi, "1");
  assert.equal(manifest.runtime, "declarative");
});

test("every merchant and shopper text ships in English, German and Spanish", () => {
  const texts = localized(manifest, "");
  assert.ok(texts.length > 0);
  for (const [path, value] of texts) {
    for (const language of LANGUAGES) assert.ok(value[language]?.trim(), `${path} needs ${language}`);
  }
});

test("views, surfaces and routes reference declared actions, entities and views", () => {
  const actions = new Set((manifest.actions ?? []).map((a) => a.name));
  const entities = new Set((manifest.entities ?? []).map((e) => e.name));
  const views = new Set((manifest.views ?? []).map((v) => v.id));
  for (const view of manifest.views ?? []) {
    for (const block of view.blocks) {
      if (block.entity) assert.ok(entities.has(block.entity), `${view.id}/${block.id} entity ${block.entity}`);
      for (const key of ["readAction", "writeAction"]) if (block[key]) assert.ok(actions.has(block[key]), `${view.id}/${block.id} ${key}`);
      for (const steps of Object.values(block.handlers ?? {})) for (const step of steps) if (step.action) assert.ok(actions.has(step.action), step.action);
    }
  }
  for (const surface of manifest.surfaces ?? []) {
    assert.ok(views.has(surface.uiPath.replace(/^native\//, "")), `${surface.id} uiPath ${surface.uiPath}`);
    for (const action of surface.actions ?? []) assert.ok(actions.has(action), `${surface.id} action ${action}`);
  }
  for (const route of manifest.apiRoutes ?? []) assert.ok(actions.has(route.action), `route ${route.path}`);
  for (const action of manifest.actions ?? []) if (action.entity) assert.ok(entities.has(action.entity), `${action.name} entity`);
});

test("public actions only read, and every write names its permission", () => {
  for (const action of manifest.actions ?? []) {
    if (action.public) assert.equal(action.readOnly, true, `${action.name} is public, so it must be read-only`);
    if (!action.readOnly) assert.ok(action.permission, `${action.name} writes, so it names a permission`);
  }
  for (const entity of manifest.entities ?? []) {
    for (const field of entity.fields) if (field.coreReference) assert.equal(field.indexed, true, `${entity.name}.${field.name}`);
  }
});

test("deploy sends environment, import and stage in order with the personal session", async () => {
  const calls = [];
  const fake = async (url, init) => {
    calls.push([new URL(url).pathname, init.headers, JSON.parse(init.body)]);
    const body = calls.length === 1 ? { id: "stage-1" } : calls.length === 2 ? { id: "build-1", digest: "sha256:abc" } : { installed: true };
    return new Response(JSON.stringify(body), { status: 200, headers: { "content-type": "application/json" } });
  };
  const result = await deploy({ manifest, url: "http://127.0.0.1:8787/x", shop: "my-shop", token: "t", fetch: fake, log: () => {} });
  assert.deepEqual(calls.map((c) => c[0]), ["/api/environments", "/api/developer/import", "/api/developer/builds/build-1/stage"]);
  assert.ok(calls.every((c) => c[1]["x-tenant"] === "my-shop" && c[1].authorization === "Bearer t"));
  assert.equal(calls[1][2].environment, "stage-1");
  assert.equal(calls[1][2].manifest.id, manifest.id);
  assert.deepEqual(calls[2][2], { approve: true, digest: "sha256:abc" });
  assert.deepEqual(result, { environment: "stage-1", build: "build-1", digest: "sha256:abc" });
  assert.equal(dryRun(manifest, [], () => {}).length, 3);
});

test("flows are saved in the sandbox, not in the live shop", async () => {
  const calls = [];
  const fake = async (url, init) => {
    calls.push([new URL(url).pathname, init.headers["x-tenant"], init.method, JSON.parse(init.body)]);
    const body = calls.length === 1 ? { id: "build-1", digest: "d" } : { saved: true };
    return new Response(JSON.stringify(body), { status: 200 });
  };
  const flows = [{ id: "my_flow", data: { event: "order.placed" } }];
  await deploy({ manifest, url: "http://127.0.0.1:8787", shop: "live", token: "t", sandbox: "stage-1", flows, fetch: fake, log: () => {} });
  assert.deepEqual(calls.at(-1), ["/api/automation/flows/my_flow", "stage-1", "PUT", { data: { event: "order.placed" }, revision: 0 }]);
  assert.ok(calls.slice(0, -1).every((c) => c[1] === "live"));
});

test("deploy refuses to run without a shop and personal session, and reports server problems", async () => {
  await assert.rejects(deploy({ manifest, url: "http://127.0.0.1:8787", shop: "", token: "" }), /VEYNDO_SHOP/);
  const refused = async () => new Response(JSON.stringify({ errors: [{ kind: "validation_error", detail: "Unknown action", pointer: "/views/0" }] }), { status: 400 });
  await assert.rejects(deploy({ manifest, url: "http://127.0.0.1:8787", shop: "s", token: "t", sandbox: "stage-1", fetch: refused, log: () => {} }),
    /HTTP 400: validation_error: Unknown action \(\/views\/0\)/);
});
