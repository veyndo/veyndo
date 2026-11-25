/** The Studio registry admits exactly what the former rights table admitted, keeps sidebar order and renders slot contributions. */
import { lazy } from "react";
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { studioRegistry } from "../../src/admin/shell/registry/first-party";
import { createStudioRegistry } from "../../src/admin/shell/registry/registry";
import {
  allows,
  contribute,
  defineStudioExtension,
} from "../../src/admin/shell/registry/define";
import StudioSlot from "../../src/admin/shell/registry/StudioSlot";
import { StudioContext } from "../../src/admin/shell/StudioContext";
import {
  canOpenStudio,
  firstStudioTab,
} from "../../src/admin/shell/permissions";
import type { Tab } from "../../src/admin/shell/navigation";

// The rights table and special rules as they stood before the registry (oracle).
const formerRights: Record<Tab, string[]> = {
  workspace: [
    "catalog.read",
    "orders.read",
    "b2b.read",
    "channels.read",
    "settings.read",
    "apps.read",
    "developer.read",
    "team.manage",
    "pos.use",
    "customers.read",
    "knowledge.read",
  ],
  setup: ["settings.write"],
  design: ["channels.read"],
  assistant: ["assistant.use"],
  overview: ["overview.read", "orders.read"],
  knowledge: ["knowledge.read"],
  agents: ["assistant.use"],
  productData: ["catalog.read"],
  orders: ["orders.read"],
  customers: ["customers.read"],
  b2b: ["b2b.read"],
  b2bQuotes: ["b2b.read"],
  pos: ["pos.use"],
  channels: ["channels.read"],
  automation: ["automation.read"],
  apps: ["apps.read"],
  storyfronts: ["apps.read"],
  developers: ["developer.read", "team.manage"],
  environments: ["environments.read"],
  users: ["team.manage"],
  commerce: ["settings.read"],
};
function formerCanOpen(tab: Tab, rights: string[]) {
  if (
    tab === "b2bQuotes" &&
    !["b2b.manage", "customers.pii"].every((right) => rights.includes(right))
  )
    return false;
  if (tab === "setup" && !rights.includes("settings.read")) return false;
  if (tab === "design" && !rights.includes("catalog.read")) return false;
  if (
    ["customers", "orders"].includes(tab) &&
    !rights.includes("customers.pii")
  )
    return false;
  if (
    ["assistant", "agents"].includes(tab) &&
    !["catalog.read", "orders.read", "knowledge.read"].every((p) =>
      rights.includes(p),
    )
  )
    return false;
  return formerRights[tab].some((p) => rights.includes(p));
}
const formerFirst = (rights: string[]) =>
  (Object.keys(formerRights) as Tab[]).find((tab) =>
    formerCanOpen(tab, rights),
  );
const formerGroups: [string, Tab[]][] = [
  [
    "intelligence",
    ["workspace", "setup", "assistant", "overview", "knowledge", "agents"],
  ],
  ["commerce", ["productData", "orders", "customers", "pos"]],
  ["experiences", ["design", "storyfronts", "channels", "automation", "apps"]],
  ["workspace", ["developers", "environments", "users", "commerce"]],
];

const allRights = [
  ...new Set([
    ...Object.values(formerRights).flat(),
    "b2b.manage",
    "customers.pii",
    "catalog.write",
  ]),
].sort();
// Deterministic pseudo-random rights sets plus every single right and the full set.
function rightSets() {
  const sets: string[][] = [[], allRights, ...allRights.map((r) => [r])];
  let seed = 7;
  for (let i = 0; i < 600; i++) {
    seed = (seed * 1103515245 + 12345) % 2 ** 31;
    sets.push(
      allRights.filter(
        (_, bit) => (seed >> bit) & 1 || (seed >> (bit + 3)) % 5 === 0,
      ),
    );
  }
  return sets;
}

describe("first-party registry", () => {
  const tabs = Object.keys(formerRights) as Tab[];

  it("registers every Studio route exactly once in the former order", () => {
    expect(studioRegistry.routes.map((route) => route.id)).toEqual(tabs);
  });

  it("admits, picks the first route and groups the sidebar exactly like the former table", () => {
    for (const rights of rightSets()) {
      for (const tab of tabs)
        expect([tab, rights, canOpenStudio(tab, rights)]).toEqual([
          tab,
          rights,
          formerCanOpen(tab, rights),
        ]);
      expect(firstStudioTab(rights)).toBe(formerFirst(rights));
      expect(
        studioRegistry
          .navigation(rights)
          .map((entry) => [entry.group, entry.routes.map((route) => route.id)]),
      ).toEqual(
        formerGroups
          .map(([group, ids]) => [
            group,
            ids.filter((id) => formerCanOpen(id, rights)),
          ])
          .filter(([, ids]) => ids.length),
      );
    }
  });

  it("registers the report board as a dashboard card", () => {
    expect(
      studioRegistry
        .contributions("admin.dashboard", [])
        .map((c) => [c.id, c.owner]),
    ).toEqual([["reports.today", "reports"]]);
  });
});

describe("registry rules", () => {
  const View = lazy(async () => ({ default: () => <p>view</p> }));
  const Card = (label: string) =>
    lazy(async () => ({
      default: ({ context }: { context: { orderId: string } }) => (
        <p>
          {label} {context.orderId}
        </p>
      ),
    }));

  it("evaluates all and any rights", () => {
    expect(allows(undefined, [])).toBe(true);
    expect(allows({ any: ["a", "b"] }, ["b"])).toBe(true);
    expect(allows({ all: ["x"], any: ["a"] }, ["a"])).toBe(false);
    expect(allows({ all: ["x"], any: ["a"] }, ["a", "x"])).toBe(true);
  });

  it("refuses duplicates, routes nobody can open and contributions at foreign locations", () => {
    const route = {
      id: "a",
      icon: "box" as const,
      rights: { any: ["r"] },
      component: View,
    };
    expect(() =>
      createStudioRegistry([
        defineStudioExtension({ id: "x" }),
        defineStudioExtension({ id: "x" }),
      ]),
    ).toThrow("Duplicate Studio extension x");
    expect(() =>
      createStudioRegistry([
        defineStudioExtension({ id: "x", routes: [route] }),
        defineStudioExtension({ id: "y", routes: [route] }),
      ]),
    ).toThrow("Duplicate Studio route a");
    expect(() =>
      createStudioRegistry([
        defineStudioExtension({
          id: "x",
          routes: [{ ...route, rights: { any: [] } }],
        }),
      ]),
    ).toThrow("admits nobody");
    const card = contribute({
      id: "c",
      kind: "dashboard-card",
      location: "admin.order",
      component: Card("c"),
    });
    expect(() =>
      createStudioRegistry([
        defineStudioExtension({ id: "x", contributions: [card] }),
      ]),
    ).toThrow("dashboard-card c cannot use admin.order");
    const slot = contribute({ ...card, kind: "page-slot" });
    expect(() =>
      createStudioRegistry([
        defineStudioExtension({ id: "x", contributions: [slot] }),
        defineStudioExtension({ id: "y", contributions: [slot] }),
      ]),
    ).toThrow("Duplicate Studio contribution c");
  });

  it("orders contributions and filters them by rights", () => {
    const registry = createStudioRegistry([
      defineStudioExtension({
        id: "x",
        contributions: [
          contribute({
            id: "late",
            kind: "page-slot",
            location: "admin.order",
            order: 2,
            component: Card("late"),
          }),
          contribute({
            id: "first",
            kind: "page-slot",
            location: "admin.order",
            component: Card("first"),
          }),
          contribute({
            id: "private",
            kind: "page-slot",
            location: "admin.order",
            rights: { any: ["orders.manage"] },
            component: Card("private"),
          }),
        ],
      }),
    ]);
    expect(registry.contributions("admin.order", []).map((c) => c.id)).toEqual([
      "first",
      "late",
    ]);
    expect(
      registry.contributions("admin.order", ["orders.manage"]).map((c) => c.id),
    ).toEqual(["first", "private", "late"]);
    expect(registry.route("a" as never)).toBeUndefined();
    expect(registry.canOpen("a" as never, ["r"])).toBe(false);
    expect(registry.firstRoute(["r"])).toBeUndefined();
  });
});

vi.mock("../../src/admin/reports/TodayReports", () => ({
  default: () => <p>today board</p>,
}));

describe("StudioSlot", () => {
  it("renders first-party dashboard cards without a mounted Studio", async () => {
    render(<StudioSlot location="admin.dashboard" context={{}} />);
    expect(await screen.findByText("today board")).toBeInTheDocument();
  });

  it("renders app locations inside a mounted Studio without first-party contributions", () => {
    const studio = { access: ["orders.read"] } as never;
    const view = render(
      <StudioContext.Provider value={studio}>
        <StudioSlot location="admin.order" context={{ orderId: "o-1" }} />
      </StudioContext.Provider>,
    );
    expect(view.container).toBeEmptyDOMElement();
  });
});
