/** Architecture gate rejects real source regressions, including cycles and ownership violations. */
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
const directory = fs.mkdtempSync(
  path.join(os.tmpdir(), "commerce-architecture-"),
);
const guard = path.resolve(import.meta.dirname, "../scripts/architecture.mjs");
function file(name, text) {
  const target = path.join(directory, name);
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, text);
  fs.writeFileSync(
    path.join(path.dirname(target), "README.md"),
    "# Fixture contract\n",
  );
}
function check() {
  return spawnSync(process.execPath, [guard, directory], { encoding: "utf8" });
}
try {
  file(
    "shared/api/base.ts",
    "/** Shared fixture contract. */\nexport const value=1;\n",
  );
  file(
    "admin/orders/view.ts",
    '/** Consumer fixture. */\nimport {value} from "../../shared/api/base";\nexport const read=()=>value;\n',
  );
  assert.equal(check().status, 0);
  file(
    "shared/api/base.ts",
    '/** Forbidden app dependency. */\nimport {read} from "../../admin/orders/view";\nexport const value=read();\n',
  );
  assert.match(check().stderr, /shared boundary/);
  assert.match(check().stderr, /Runtime import cycle/);
  file("shared/api/base.ts", '/** Missing import. */\nimport "./missing";\n');
  assert.match(check().stderr, /cannot resolve/);
  file("shared/api/base.ts", "export const missingComment=1;\n");
  assert.match(check().stderr, /responsibility comment/);
  file(
    "shared/api/base.ts",
    "/** Documented fixture. */\nexport const value=1;\n",
  );
  const sdk = path.resolve(import.meta.dirname, "../../extensions/sdk");
  file(
    "shared/api/base.ts",
    `/** Shared immutable native asset contract. */\nimport contract from ${JSON.stringify(path.relative(path.join(directory, "shared/api"), path.join(sdk, "native-asset-actions.json")))};\nexport const actions=contract;\n`,
  );
  assert.equal(check().status, 0, check().stderr);
  file(
    "shared/api/base.ts",
    `/** Other SDK implementation remains outside the boundary. */\nimport client from ${JSON.stringify(path.relative(path.join(directory, "shared/api"), path.join(sdk, "browser.js")))};\nexport const actions=client;\n`,
  );
  assert.match(check().stderr, /outside the source ownership tree/);
  file(
    "shared/api/base.ts",
    "/** Documented fixture. */\nexport const value=1;\n",
  );
  file("kit/tokens/index.ts", "/** Token fixture. */\nexport const space=4;\n");
  file(
    "kit/ui/index.ts",
    '/** Kit fixture. */\nimport {space} from "../tokens";\nexport const gap=space;\n',
  );
  file(
    "kit/ui/ui.css",
    "/* Kit fixture styles. */\n.vy-a { color: var(--vy-color-text); }\n",
  );
  file(
    "admin/orders/view.ts",
    '/** Consumer fixture. */\nimport {gap} from "../../kit/ui";\nexport const read=()=>gap;\n',
  );
  assert.equal(check().status, 0, check().stderr);
  file("kit/ui/ui.css", "/* Kit fixture styles. */\n.vy-a { color: #fff; }\n");
  assert.match(check().stderr, /colour literal/);
  file(
    "kit/ui/ui.css",
    "/* Kit fixture styles. */\n.vy-a { color: rgb(0 0 0); }\n",
  );
  assert.match(check().stderr, /colour literal/);
  file(
    "kit/ui/ui.css",
    "/* Kit fixture styles. */\n.vy-a { color: var(--vy-color-text); }\n",
  );
  file("kit/ui/Button.ts", "/** Kit internal. */\nexport const b=1;\n");
  file(
    "admin/orders/view.ts",
    '/** Consumer fixture. */\nimport {b} from "../../kit/ui/Button";\nexport const read=()=>b;\n',
  );
  assert.match(check().stderr, /internals; import its index/);
  file(
    "admin/orders/view.ts",
    "/** Consumer fixture. */\nexport const read=()=>1;\n",
  );
  file(
    "admin/orders/view.ts",
    '/** Consumer fixture. */\nimport css from "../../kit/ui/ui.css?inline";\nexport const read=()=>css;\n',
  );
  assert.match(check().stderr, /internals; import its index/);
  file(
    "admin/orders/view.ts",
    "/** Consumer fixture. */\nexport const read=()=>1;\n",
  );
  file(
    "kit/tokens/index.ts",
    '/** Token fixture importing upward. */\nimport {gap} from "../ui";\nexport const space=gap;\n',
  );
  assert.match(check().stderr, /kit layering/);
  file(
    "kit/tokens/index.ts",
    '/** Token fixture importing the app. */\nimport {value} from "../../shared/api/base";\nexport const space=value;\n',
  );
  assert.match(check().stderr, /imports outside kit/);
  file("kit/tokens/index.ts", "/** Token fixture. */\nexport const space=4;\n");
  assert.equal(check().status, 0, check().stderr);
  file(
    "admin/orders/OrdersRoute.ts",
    '/** Route view reading the registry. */\nimport {routes} from "../shell/registry";\nexport const view=()=>routes;\n',
  );
  file(
    "admin/shell/registry.ts",
    '/** Registry fixture. */\nimport extension from "../orders/studio-extension";\nexport const routes=[extension];\n',
  );
  file(
    "admin/orders/studio-extension.ts",
    '/** Descriptor registering its view lazily. */\nexport default {load:()=>import("./OrdersRoute")};\n',
  );
  assert.equal(check().status, 0, check().stderr);
  file(
    "admin/orders/studio-extension.ts",
    '/** Descriptor importing its view statically. */\nimport {view} from "./OrdersRoute";\nexport default {view};\n',
  );
  assert.match(check().stderr, /Runtime import cycle/);
  file(
    "admin/orders/other.ts",
    '/** Ordinary module with a lazy cycle. */\nexport const load=()=>import("../shell/registry");\n',
  );
  file(
    "admin/orders/studio-extension.ts",
    '/** Descriptor registering its view lazily. */\nexport default {load:()=>import("./OrdersRoute")};\n',
  );
  file(
    "admin/shell/registry.ts",
    '/** Registry fixture. */\nimport {load} from "../orders/other";\nexport const routes=[load];\n',
  );
  assert.match(check().stderr, /Runtime import cycle/);
  for (const name of [
    "admin/orders/OrdersRoute.ts",
    "admin/shell/registry.ts",
    "admin/orders/studio-extension.ts",
    "admin/orders/other.ts",
  ])
    fs.unlinkSync(path.join(directory, name));
  file(
    "admin/orders/orders.css",
    "/* Area styles. */\n.a { color: var(--vy-color-text); }\n",
  );
  assert.equal(check().status, 0, check().stderr);
  file(
    "admin/orders/orders.css",
    "/* Area styles. */\n.a { color: #142432; }\n",
  );
  assert.match(check().stderr, /frontend styles use var/);
  file(
    "storefront/shell/shop.css",
    "/* Shop styles. */\n.a { background: rgba(0,0,0,.1); }\n",
  );
  assert.match(
    check().stderr,
    /storefront\/shell\/shop.css uses a colour literal/,
  );
  fs.unlinkSync(path.join(directory, "storefront/shell/shop.css"));
  file(
    "admin/orders/orders.css",
    "/* Area styles. */\n.a { color: var(--vy-color-text); }\n",
  );
  file(
    "admin/orders/view.ts",
    '/** Consumer fixture. */\nexport const cls="studio-primary";\n',
  );
  assert.match(check().stderr, /retired Studio class/);
  file(
    "shared/api/base.ts",
    '/** Shared fixture. */\nexport const value="x studio-card";\n',
  );
  assert.match(check().stderr, /retired Studio class/);
  file(
    "shared/api/base.ts",
    "/** Documented fixture. */\nexport const value=1;\n",
  );
  file(
    "admin/orders/view.ts",
    "/** Consumer fixture. */\nexport const read=()=>1;\n",
  );
  assert.equal(check().status, 0, check().stderr);
  file(
    "shared/api/types.ts",
    "/** Request fixture. */\nexport type RequestFn=(p:string)=>void;\nexport type ApiPath=string;\n",
  );
  file(
    "admin/orders/view.ts",
    '/** Consumer fixture. */\nimport type { RequestFn } from "../../shared/api/types";\nexport const read=(r:RequestFn)=>r("/x");\n',
  );
  assert.match(check().stderr, /open app request contract/);
  file(
    "admin/orders/view.ts",
    '/** Consumer fixture. */\nimport type { ApiPath } from "../../shared/api/types";\nexport const p="/x" as ApiPath;\n',
  );
  assert.match(check().stderr, /casts a path to ApiPath/);
  file(
    "admin/orders/view.ts",
    "/** Consumer fixture. */\nexport const read=()=>1;\n",
  );
  assert.equal(check().status, 0, check().stderr);
  fs.unlinkSync(path.join(directory, "shared/api/README.md"));
  assert.match(check().stderr, /folder contract/);
  console.log(
    "PASS source ownership, runtime-cycle, missing-import, comment, documentation, kit layering/token, lazy-registration, Studio kit-only and catalogued-path negative controls",
  );
} finally {
  fs.rmSync(directory, { recursive: true, force: true });
}
