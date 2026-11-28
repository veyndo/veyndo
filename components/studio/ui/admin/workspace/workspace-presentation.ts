/** Project the native workspace's reviewed controls through the shared A2UI adapter into documents for the catalog renderer. */
import {
  createA2uiAdapter,
  type A2uiProjection,
  type RendererCapabilities,
} from "@veyndo/sdk/a2ui";
import { createLayoutRegistry } from "@veyndo/sdk/composition";
import { admitWorkspaceView } from "../../../../../frontend/src/shared/grid/workspace-view";

const catalogId = "veyndo:studio-workspace-v1";
const bindings = {
  Directory: { id: "Directory", title: "Directory", version: 1 },
  Intake: { id: "Intake", title: "Intake", version: 1 },
  ShippingPolicy: {
    id: "ShippingPolicy",
    title: "Shipping policy",
    version: 1,
  },
} as const;
type Control = keyof typeof bindings;

const adapter = createA2uiAdapter({
  catalogId,
  rootComponent: "Workspace",
  views: Object.values(bindings).map(({ id, version }) => ({
    view: id,
    version,
    component: id,
  })),
});
const nativeCapabilities = { "1.0": { supportedCatalogIds: [catalogId] } };

function readNativeControls(projection: A2uiProjection): Control[] | null {
  if (projection.kind !== "a2ui") return null;
  const update = projection.messages.find(
    (message) => "updateComponents" in message,
  );
  const components = update?.updateComponents.components;
  const root = components?.find((component) => component.id === "root");
  if (root?.component !== "Workspace" || !root.children) return null;
  const controls: Control[] = [];
  for (const id of root.children) {
    const name = components?.find(
      (component) => component.id === id,
    )?.component;
    if (!name || !Object.hasOwn(bindings, name)) return null;
    controls.push(name as Control);
  }
  return controls;
}

export const workspaceCatalogId = catalogId;

function project(
  candidate: unknown,
  rights: string[],
  capabilities: RendererCapabilities,
) {
  const view = admitWorkspaceView(candidate, rights);
  if (!view) return null;
  const selected = (view.components ?? ["records"]).flatMap(
    (component): Control[] => {
      switch (component) {
        case "records":
          return ["Directory"];
        case "intake":
          return rights.includes("catalog.write") ? ["Intake"] : [];
        case "shippingPolicy":
          return ["ShippingPolicy"];
        case "details":
          return [];
      }
    },
  );
  const registry = createLayoutRegistry(selected.map((name) => bindings[name]));
  const layout = {
    version: 1,
    panels: selected.map((name) => ({ id: name, view: name, version: 1 })),
  };
  const presentation = adapter.project(registry, layout, {
    surfaceId: "studio_workspace",
    capabilities,
  });
  return presentation.kind === "rejected" ? null : presentation;
}

/** The admitted layout as a document for the shared catalog renderer: A2UI v1.0, or OpenUI Lang for clients without the catalog. */
export function workspaceDocument(
  candidate: unknown,
  rights: string[],
  capabilities: RendererCapabilities = nativeCapabilities,
) {
  const presentation = project(candidate, rights, capabilities);
  if (!presentation) return null;
  if (presentation.kind === "a2ui" && readNativeControls(presentation))
    return { protocol: "a2ui-1.0", document: presentation.messages } as const;
  return {
    protocol: "openui",
    document: `root = Workspace([${presentation.panels.map((panel) => `${panel.view}()`).join(", ")}])`,
  } as const;
}

export function composeNativeWorkspace(
  candidate: unknown,
  rights: string[],
  capabilities: RendererCapabilities = nativeCapabilities,
) {
  const presentation = project(candidate, rights, capabilities);
  if (!presentation) return null;
  const controls = readNativeControls(presentation);
  if (controls) return { protocol: "a2ui-1.0", controls } as const;
  return {
    protocol: "openui",
    source: `root = Workspace([${presentation.panels.map((panel) => `${panel.view}()`).join(", ")}])`,
  } as const;
}
