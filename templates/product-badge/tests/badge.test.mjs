/** Product-badge specifics: the public product view only reads badges, settings stay merchant-editable and share the badge styles. */
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";

const manifest = JSON.parse(await readFile(new URL("../manifest.json", import.meta.url), "utf8"));
const entity = (name) => manifest.entities.find((e) => e.name === name);
const surface = (location) => manifest.surfaces.find((s) => s.location === location);
const action = (name) => manifest.actions.find((a) => a.name === name);

test("shoppers see badges on the product page and can only read them", () => {
  const shopper = surface("product.detail");
  assert.deepEqual(shopper.actions, ["list_badges"]);
  assert.equal(action("list_badges").public, true);
  assert.equal(entity("badges").publicRead, true);
  assert.equal(entity("badges").fields.find((f) => f.name === "product_id").coreReference, "product");
});

test("editing a badge or the settings needs catalog.write", () => {
  for (const name of ["save_badges", "save_settings"]) {
    assert.equal(action(name).public, false);
    assert.equal(action(name).permission, "catalog.write");
  }
  assert.equal(surface("admin.navigation").uiPath, "native/settings");
  assert.equal(entity("settings").publicRead, false, "settings are merchant-only");
});

test("the default style is one of the badge styles", () => {
  const styles = (name, field) => entity(name).fields.find((f) => f.name === field).choices.map((c) => c.value);
  assert.deepEqual(styles("settings", "default_tone"), styles("badges", "tone"));
});
