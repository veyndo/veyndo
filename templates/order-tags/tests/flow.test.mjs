/** Order-tag specifics: the flow calls an action this app declares for flows, on an order event, with a tag the app knows. */
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";

const read = async (path) => JSON.parse(await readFile(new URL(path, import.meta.url), "utf8"));
const manifest = await read("../manifest.json");
const flow = await read("../flows/tag_large_orders.json");
const action = (name) => manifest.actions.find((a) => a.name === name);
const tags = manifest.entities.find((e) => e.name === "tags").fields.find((f) => f.name === "tag").choices.map((c) => c.value);

test("the flow targets this app and a flow-allowed action", () => {
  assert.equal(flow.appAction.app, manifest.id);
  assert.equal(flow.action, "app_action");
  assert.equal(action(flow.appAction.action).flowAllowed, true);
});

test("order events bind the order: the action declares orderId, the flow leaves it to the host", () => {
  assert.match(flow.event, /^order\./);
  const schema = action(flow.appAction.action).inputSchema;
  assert.ok(schema.properties.orderId, "the host binds the canonical order ID only into a declared orderId");
  assert.equal(flow.appAction.arguments.orderId, undefined);
});

test("suggested tags are the tags staff can set", () => {
  assert.deepEqual(action("tag_suggested").inputSchema.properties.tag.enum, tags);
  assert.ok(tags.includes(flow.appAction.arguments.tag));
});

test("tags stay private to the merchant", () => {
  assert.equal(manifest.entities[0].publicRead, false);
  assert.ok(manifest.actions.every((a) => !a.public));
  assert.equal(action("save_tags").permission, "orders.write");
});

test("the flow name and instruction ship in English, German and Spanish", () => {
  for (const text of [flow.name, flow.instruction]) for (const language of ["en", "de", "es"]) assert.ok(text[language]);
});
