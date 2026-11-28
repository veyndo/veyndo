/** Deploy this app to a private Veyndo sandbox: create (or reuse) the environment, import the Manifest as a build, stage that exact digest, then save its flows there. */
// Identical in every app template (templates/shared/scripts/deploy-sandbox.mjs; scripts/templates.py checks it).
// Usage:  VEYNDO_URL=https://api.veyndo.ai VEYNDO_SHOP=<shop id> VEYNDO_TOKEN=<personal session> node scripts/deploy-sandbox.mjs
// Optional: VEYNDO_SANDBOX=<existing sandbox id> to reuse one; --dry-run prints the requests without sending them.
// A sandbox is private to you and never touches the live shop, its orders or its customers.
import { readdir, readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

/** The calls, in order, with the shop and credential that each one uses. `flows` are `{ id, data }` from flows/*.json. */
export async function deploy({ manifest, url, shop, token, sandbox, flows = [], fetch: send = fetch, log = console.log }) {
  if (!url || !shop || !token) throw new Error("Set VEYNDO_URL, VEYNDO_SHOP and VEYNDO_TOKEN (a personal session, never an integration key).");
  const origin = new URL(url).origin;
  const call = async (path, body, tenant = shop, method = "POST") => {
    const response = await send(origin + path, {
      method,
      headers: { authorization: `Bearer ${token}`, "x-tenant": tenant, "content-type": "application/json", accept: "application/json" },
      body: JSON.stringify(body),
    });
    const text = await response.text();
    const value = text ? JSON.parse(text) : {};
    if (!response.ok) {
      const detail = (value.errors ?? []).map((e) => `${e.kind ?? e.code}: ${e.detail}${e.pointer ? ` (${e.pointer})` : ""}`).join("; ");
      throw new Error(`${path} failed with HTTP ${response.status}${detail ? `: ${detail}` : ""}`);
    }
    return value;
  };
  const environment = sandbox || (await call("/api/environments", { name: `${manifest.name.en} sandbox` })).id;
  log(`Sandbox: ${environment}`);
  const build = await call("/api/developer/import", {
    environment,
    prompt: `Deploy template app ${manifest.id} ${manifest.version}`,
    summary: manifest.name,
    manifest,
  });
  log(`Build: ${build.id} (${build.digest})`);
  await call(`/api/developer/builds/${encodeURIComponent(build.id)}/stage`, { approve: true, digest: build.digest });
  log(`Installed ${manifest.id} ${manifest.version} in sandbox ${environment}.`);
  // Flows are saved in the sandbox itself, never in the live shop. A rerun conflicts on revision 0; edit it in Studio instead.
  for (const flow of flows) {
    await call(`/api/automation/flows/${encodeURIComponent(flow.id)}`, { data: flow.data, revision: 0 }, environment, "PUT");
    log(`Flow ${flow.id} saved in the sandbox.`);
  }
  log(`Open ${origin}/#merchant, choose the sandbox "${manifest.name.en} sandbox" in the top bar, then Apps.`);
  return { environment, build: build.id, digest: build.digest };
}

/** Prints what would be sent: no network, no credential needed. */
export function dryRun(manifest, flows = [], log = console.log) {
  const steps = ["POST /api/environments", "POST /api/developer/import", "POST /api/developer/builds/{id}/stage",
    ...flows.map((flow) => `PUT /api/automation/flows/${flow.id} (in the sandbox)`)];
  for (const step of steps) log(step);
  log(`Manifest ${manifest.id} ${manifest.version}`);
  return steps;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const manifest = JSON.parse(await readFile(new URL("../manifest.json", import.meta.url), "utf8"));
  const folder = new URL("../flows/", import.meta.url);
  const names = await readdir(folder).catch(() => []);
  const flows = await Promise.all(names.filter((n) => n.endsWith(".json")).sort().map(async (name) => ({
    id: name.slice(0, -5), data: JSON.parse(await readFile(new URL(name, folder), "utf8")),
  })));
  if (process.argv.includes("--dry-run")) dryRun(manifest, flows);
  else {
    const env = process.env;
    try {
      await deploy({ manifest, url: env.VEYNDO_URL, shop: env.VEYNDO_SHOP, token: env.VEYNDO_TOKEN, sandbox: env.VEYNDO_SANDBOX, flows });
    } catch (error) {
      console.error(error.message);
      process.exitCode = 1;
    }
  }
}
