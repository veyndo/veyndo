/** Shopper-guide specifics: the assistant view exposes only public read tools; merchant notes stay behind a merchant permission. */
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";

const manifest = JSON.parse(await readFile(new URL("../manifest.json", import.meta.url), "utf8"));
const action = (name) => manifest.actions.find((a) => a.name === name);
const surface = (location) => manifest.surfaces.find((s) => s.location === location);

test("the shopping assistant view calls public, read-only MCP tools", () => {
  const view = surface("storefront.assistant");
  assert.ok(view, "a storefront.assistant surface");
  for (const name of view.actions) {
    assert.equal(action(name).public, true, name);
    assert.equal(action(name).readOnly, true, name);
    assert.equal(action(name).mcp, true, name);
  }
});

test("merchant results need a merchant permission and never become public", () => {
  const view = surface("admin.assistant");
  assert.ok(view.permission);
  for (const name of view.actions) assert.notEqual(action(name).public, true, name);
  const notes = manifest.entities.find((e) => e.name === "merchandising");
  assert.equal(notes.publicRead, false);
});

test("writes are never MCP tools of the shopper", () => {
  for (const a of manifest.actions.filter((x) => !x.readOnly)) assert.notEqual(a.mcp && a.public, true, a.name);
});
