/** The catalog renderer: every example renders accessibly in every shipped language; untrusted documents are refused visibly and in place. */
import { fireEvent, render, screen } from "@testing-library/react";
import axe from "axe-core";
import { describe, expect, it, vi } from "vitest";
import {
  CatalogView,
  catalogId,
  entries,
  entryByName,
  exampleAdmission,
  exampleDocument,
  galleryLocales,
  readDocument,
  sampleText,
  surfaces,
  validateDocument,
  type Admission,
} from "../../src/kit/catalog";
import { tokens } from "../../src/kit/tokens";

const studio: Admission = { surface: "studio", locale: "en-GB" };
const tree = (root: unknown) => ({
  format: "veyndo-ui/1",
  catalog: catalogId,
  root,
});
const refusals = (input: unknown, admission: Admission = studio) => {
  const result = validateDocument(input, admission);
  return result.problems.map((problem) => problem.code);
};

/** axe on the rendered DOM. Contrast needs layout and runs in the nightly browser gallery check. */
async function violations(container: HTMLElement) {
  const result = await axe.run(container, {
    rules: { "color-contrast": { enabled: false }, region: { enabled: false } },
  });
  return result.violations.map(
    (violation) =>
      `${violation.id}: ${violation.nodes.map((node) => node.html).join(" | ")}`,
  );
}

describe("catalog entries", () => {
  it("declare a complete contract", () => {
    for (const entry of entries) {
      expect(entry.description.length, entry.name).toBeGreaterThan(10);
      expect(entry.a11y.notes.length, entry.name).toBeGreaterThan(10);
      expect(
        entry.surfaces.every((surface) => surfaces.includes(surface)),
        entry.name,
      ).toBe(true);
      expect(Boolean(entry.codeOnly), entry.name).toBe(
        entry.surfaces.length === 0,
      );
      for (const rule of Object.values(entry.slots ?? {}))
        if (Array.isArray(rule.accepts))
          for (const name of rule.accepts)
            expect(
              entryByName(name),
              `${entry.name} accepts ${name}`,
            ).toBeDefined();
      for (const key of entry.hostData ?? [])
        expect(Object.keys(entry.props.shape), entry.name).toContain(key);
      if (typeof entry.a11y.name === "object")
        expect(Object.keys(entry.props.shape), entry.name).toContain(
          entry.a11y.name.prop,
        );
    }
  });

  it("show every renderable entry in at least one example", () => {
    const shown = new Set<string>();
    const walk = (node: {
      component: string;
      children?: unknown[];
      slots?: Record<string, unknown[]>;
    }) => {
      shown.add(node.component);
      for (const child of [
        ...(node.children ?? []),
        ...Object.values(node.slots ?? {}).flat(),
      ])
        walk(child as never);
    };
    for (const entry of entries)
      for (const example of entry.examples)
        walk(example.node(sampleText("en-GB")));
    const missing = entries
      .filter((entry) => entry.surfaces.length && !shown.has(entry.name))
      .map((entry) => entry.name);
    // These need real host media or signed-in customer data to show anything meaningful.
    expect(missing.sort()).toEqual([
      "AccountOverview",
      "Image",
      "OrderList",
      "ProductSummary",
      "VisuallyHidden",
    ]);
  });

  it("use only sample text the vocabulary has, in every shipped language", () => {
    for (const locale of galleryLocales) {
      const missing: string[] = [];
      const known = sampleText(locale);
      const t = (key: string) => {
        const value = known(key);
        if (value === key) missing.push(key);
        return value;
      };
      for (const entry of entries)
        for (const example of entry.examples) {
          example.node(t);
          example.data?.(t);
        }
      expect(missing, locale).toEqual([]);
    }
  });

  it("follow the spacing scale of the design tokens", () => {
    expect(entryByName("Stack")!.props.shape.gap.unwrap().options).toEqual(
      Object.keys(tokens.space),
    );
  });
});

describe.each(
  entries
    .filter((entry) => entry.examples.length)
    .map((entry) => [entry.name, entry] as const),
)("%s", (_, entry) => {
  it.each(entry.examples.map((example) => [example.id, example] as const))(
    "renders example %s in every language without refusals or axe violations",
    async (__, example) => {
      for (const locale of galleryLocales) {
        const { container, unmount } = render(
          <CatalogView
            document={exampleDocument(example, locale)}
            admission={exampleAdmission(entry, example, locale)}
            onAction={() => undefined}
          />,
        );
        expect(container.querySelector("[data-refusal]"), locale).toBeNull();
        const root = example.node(sampleText(locale));
        const role = entry.a11y.role;
        if (root.component === entry.name && role && /^[a-z]+$/.test(role)) {
          const name =
            typeof entry.a11y.name === "object"
              ? root.props?.[entry.a11y.name.prop]
              : undefined;
          expect(
            screen.getAllByRole(role, typeof name === "string" ? { name } : {})
              .length,
            `${entry.name} ${role} ${String(name)}`,
          ).toBeGreaterThan(0);
        }
        expect(
          await violations(container),
          `${entry.name}/${example.id} ${locale}`,
        ).toEqual([]);
        unmount();
      }
    },
  );
});

describe("safe rendering", () => {
  it("refuses unknown components in place with a localized notice and renders the rest", () => {
    const { container } = render(
      <CatalogView
        document={tree({
          component: "Stack",
          children: [
            { component: "Text", props: { text: "Visible" } },
            { component: "Marquee", props: { text: "x" } },
          ],
        })}
        admission={{ surface: "storefront", locale: "de-DE" }}
      />,
    );
    expect(screen.getByText("Visible")).toBeInTheDocument();
    expect(
      container.querySelector('[data-refusal="unknown-component"]'),
    ).not.toBeNull();
    expect(screen.getByRole("status")).toHaveTextContent(
      "Dieser Teil kann nicht angezeigt werden",
    );
    expect(
      screen.getByText(
        "Er verwendet eine Komponente, die diese Anwendung nicht kennt.",
      ),
    ).toBeInTheDocument();
  });

  it("refuses invalid and unknown props", () => {
    expect(
      refusals(
        tree({ component: "Button", props: { label: "Go", tone: "neon" } }),
      ),
    ).toEqual(["invalid-props"]);
    expect(refusals(tree({ component: "Badge", props: { text: "" } }))).toEqual(
      ["invalid-props"],
    );
    expect(
      refusals(tree({ component: "Badge", props: { text: "a", extra: 1 } })),
    ).toEqual(["invalid-props"]);
    expect(
      readDocument(
        tree({ component: "Badge", props: { text: "a" }, onClick: "x" }),
      ).ok,
    ).toBe(false);
  });

  it("refuses markup, script attributes and inline HTML", () => {
    expect(
      refusals(
        tree({
          component: "Text",
          props: { text: '<img src=x onerror="alert(1)">' },
        }),
      ),
    ).toEqual(["unsafe-html"]);
    expect(
      refusals(
        tree({
          component: "Text",
          props: { text: "ok", dangerouslySetInnerHTML: { __html: "<b>" } },
        }),
      ),
    ).toEqual(["unsafe-html"]);
    expect(
      refusals(
        tree({
          component: "Button",
          props: { label: "Go", onClick: "steal()" },
        }),
      ),
    ).toEqual(["unsafe-html"]);
    expect(
      refusals(
        tree({ component: "Text", props: { text: "Prices < 10 € and 2 > 1" } }),
      ),
    ).toEqual([]);
  });

  it("refuses unsafe URLs and absolute URLs outside the admitted origins", () => {
    const link = (href: string) =>
      tree({ component: "LinkButton", props: { label: "Open", href } });
    for (const href of [
      "javascript:alert(1)",
      "data:text/html,x",
      "//evil.example/x",
      "http://shop.example/",
      "https://user:pw@shop.example/",
      "/\\evil.example",
      "https://shop.example/a b",
    ])
      expect(refusals(link(href)), href).toEqual(["unsafe-url"]);
    expect(refusals(link("https://evil.example/"))).toEqual(["unsafe-url"]);
    expect(
      refusals(link("https://shop.example/p"), {
        ...studio,
        origins: ["https://shop.example"],
      }),
    ).toEqual([]);
    expect(refusals(link("/products/bag"))).toEqual([]);
    render(
      <CatalogView
        document={link("https://shop.example/p")}
        admission={{ ...studio, origins: ["https://shop.example"] }}
      />,
    );
    expect(screen.getByRole("link", { name: "Open" })).toHaveAttribute(
      "rel",
      "noopener noreferrer",
    );
  });

  it("binds only data the host admitted and refuses commerce values stated by the document", () => {
    const product = {
      id: "bag",
      name: "Bag",
      url: "/products/bag",
      price: 1,
      currency: "EUR",
    };
    const card = (value: unknown) =>
      tree({ component: "ProductCard", props: { product: value } });
    expect(refusals(card(product))).toEqual(["data-not-admitted"]);
    expect(refusals(card({ path: "/product" }))).toEqual(["data-not-admitted"]);
    expect(
      refusals(card({ path: "/__proto__/constructor" }), {
        ...studio,
        data: {},
      }),
    ).toEqual(["data-not-admitted"]);
    expect(
      refusals(card({ path: "/product" }), { ...studio, data: { product } }),
    ).toEqual([]);
    expect(
      refusals(card({ path: "/product" }), {
        ...studio,
        data: { product: { ...product, price: "free" } },
      }),
    ).toEqual(["invalid-props"]);
    expect(
      refusals(
        tree({ component: "Text", props: { text: { path: "/note" } } }),
        { ...studio, data: { note: "From the host" } },
      ),
    ).toEqual([]);
    expect(
      refusals({
        ...tree({ component: "Text", props: { text: "x" } }),
        data: { product },
      }),
    ).toEqual(["data-not-admitted"]);
    expect(
      refusals([
        { version: "1.0", createSurface: { surfaceId: "s", catalogId } },
        {
          version: "1.0",
          updateDataModel: { surfaceId: "s", path: "/product", value: product },
        },
      ]),
    ).toEqual(["data-not-admitted"]);
  });

  it("forwards only admitted actions and disables unbound controls", () => {
    const onAction = vi.fn();
    const document = tree({
      component: "Stack",
      children: [
        {
          component: "Button",
          props: { label: "Save" },
          action: { name: "save", context: { id: "p1" } },
        },
        { component: "Button", props: { label: "Idle" } },
      ],
    });
    render(
      <CatalogView
        document={document}
        admission={{ ...studio, actions: ["save"] }}
        onAction={onAction}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(onAction).toHaveBeenCalledWith({
      name: "save",
      context: { id: "p1" },
      event: "press",
      node: "root.children.0",
    });
    expect(screen.getByRole("button", { name: "Idle" })).toBeDisabled();
    expect(refusals(document)).toEqual(["action"]);
    expect(
      refusals(
        tree({ component: "Text", props: { text: "x" }, action: "save" }),
        { ...studio, actions: ["save"] },
      ),
    ).toEqual(["action"]);
    expect(
      refusals(
        tree({
          component: "Button",
          props: { label: "x" },
          action: "not valid!",
        }),
        { ...studio, actions: ["save"] },
      ),
    ).toEqual(["action"]);
  });

  it("reports form values with the admitted action", () => {
    const onAction = vi.fn();
    render(
      <CatalogView
        document={tree({
          component: "TextInput",
          props: { name: "email", label: "Email" },
          action: "field",
        })}
        admission={{ ...studio, actions: ["field"] }}
        onAction={onAction}
      />,
    );
    const input = screen.getByRole("textbox", { name: "Email" });
    fireEvent.change(input, { target: { value: "a@example.test" } });
    fireEvent.blur(input);
    expect(onAction).toHaveBeenCalledWith(
      expect.objectContaining({
        name: "field",
        event: "change",
        payload: { name: "email", value: "a@example.test" },
      }),
    );
  });

  it("enforces slots, placement and surfaces", () => {
    expect(
      refusals(
        tree({
          component: "Notice",
          props: { text: "x" },
          slots: { action: [{ component: "Text", props: { text: "y" } }] },
        }),
      ),
    ).toEqual(["slot"]);
    expect(
      refusals(
        tree({
          component: "Card",
          slots: {
            actions: Array.from({ length: 4 }, () => ({
              component: "Badge",
              props: { text: "b" },
            })),
          },
        }),
      ),
    ).toEqual(["slot"]);
    expect(
      refusals(
        tree({
          component: "Badge",
          props: { text: "b" },
          children: [{ component: "Text", props: { text: "y" } }],
        }),
      ),
    ).toEqual(["slot"]);
    expect(
      refusals(tree({ component: "TabPanel", props: { id: "a", label: "A" } })),
    ).toEqual(["placement"]);
    expect(
      refusals(
        tree({ component: "Tabs", props: { label: "T" }, children: [] }),
      ),
    ).toEqual(["slot"]);
    expect(
      refusals(tree({ component: "IconButton", props: { label: "x" } })),
    ).toEqual(["not-admitted"]);
    expect(
      refusals(tree({ component: "Dialog", props: { title: "x" } })),
    ).toEqual(["not-admitted"]);
    const account = tree({
      component: "OrderList",
      props: { orders: { path: "/orders" } },
    });
    expect(
      refusals(account, {
        surface: "mcp-app",
        locale: "en-GB",
        data: { orders: [] },
      }),
    ).toEqual(["not-admitted"]);
    expect(
      refusals(account, {
        surface: "storefront",
        locale: "en-GB",
        data: { orders: [] },
      }),
    ).toEqual([]);
  });

  it("refuses documents beyond the size, node and depth limits", () => {
    let deep: Record<string, unknown> = {
      component: "Text",
      props: { text: "x" },
    };
    for (let i = 0; i < 20; i++)
      deep = { component: "Stack", children: [deep] };
    expect(refusals(tree(deep))).toEqual(["limits"]);
    const wide = {
      component: "Stack",
      children: Array.from({ length: 8 }, () => ({
        component: "Stack",
        children: Array.from({ length: 50 }, () => ({
          component: "Badge",
          props: { text: "b" },
        })),
      })),
    };
    expect(refusals(tree(wide))).toEqual(["limits"]);
    expect(
      refusals(
        tree({ component: "Text", props: { text: "x".repeat(300000) } }),
      ),
    ).toEqual(["limits"]);
  });

  it("refuses other catalogs and unreadable input with a document-level notice", () => {
    const { container } = render(
      <CatalogView
        document={{
          format: "veyndo-ui/1",
          catalog: "acme:widgets/9",
          root: {},
        }}
        admission={{ surface: "app", locale: "es-ES" }}
      />,
    );
    expect(container.querySelector('[data-refusal="catalog"]')).not.toBeNull();
    expect(
      screen.getByText("Esta vista no se puede mostrar"),
    ).toBeInTheDocument();
    expect(refusals(42)).toEqual(["format"]);
    expect(refusals("{not json")).toEqual(["format"]);
    expect(
      refusals([
        { version: "0.8", createSurface: { surfaceId: "s", catalogId } },
      ]),
    ).toEqual(["format"]);
    expect(
      refusals([
        {
          version: "1.0",
          updateComponents: { surfaceId: "s", components: [] },
        },
      ]),
    ).toEqual(["catalog"]);
  });

  it("keeps a failing host component from breaking the view", () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const { container } = render(
      <CatalogView
        document={tree({
          component: "Stack",
          children: [
            { component: "Broken" },
            { component: "Text", props: { text: "Still here" } },
          ],
        })}
        admission={{
          ...studio,
          host: {
            Broken: {
              description: "throws",
              render: () => {
                throw Error("boom");
              },
            },
          },
        }}
      />,
    );
    expect(container.querySelector('[data-refusal="render"]')).not.toBeNull();
    expect(screen.getByText("Still here")).toBeInTheDocument();
    expect(() =>
      validateDocument(tree({ component: "Text" }), {
        ...studio,
        host: { Text: { description: "", render: () => null } },
      }),
    ).toThrow(/shadows/);
  });
});

describe("document formats", () => {
  const expected = {
    component: "Card",
    props: { title: "Delivery" },
    slots: {
      actions: [{ component: "Badge", props: { text: "New", tone: "accent" } }],
    },
    children: [
      {
        component: "Button",
        props: { label: "Save", tone: "primary" },
        action: "save",
      },
    ],
  };
  const rendered = (document: unknown) => {
    const { container, unmount } = render(
      <CatalogView
        document={document}
        admission={{ ...studio, actions: ["save"] }}
        onAction={() => undefined}
      />,
    );
    const html = container.innerHTML
      .replace(/ data-(format|node)="[^"]*"/g, "")
      .replace(/«[^»]*»|_r_[a-z0-9]+_/g, "id");
    unmount();
    return html;
  };

  it("render A2UI v1.0 and OpenUI Lang the same as veyndo-ui/1", () => {
    const a2ui = [
      { version: "1.0", createSurface: { surfaceId: "s", catalogId } },
      {
        version: "1.0",
        updateComponents: {
          surfaceId: "s",
          components: [
            {
              id: "root",
              component: "Card",
              title: "Delivery",
              actions: ["badge"],
              child: "save",
            },
            { id: "badge", component: "Badge", text: "New", tone: "accent" },
            {
              id: "save",
              component: "Button",
              label: "Save",
              tone: "primary",
              action: "save",
            },
          ],
        },
      },
    ];
    const openui =
      'root = Card([Button("Save", "primary", "md", false, false, "save")], "Delivery", 3, [Badge("New", "accent")])';
    const reference = rendered(tree(expected));
    expect(reference).toContain("Delivery");
    expect(rendered(a2ui)).toBe(reference);
    expect(rendered(openui)).toBe(reference);
    expect(rendered(JSON.stringify({ messages: a2ui }))).toBe(reference);
  });

  it("refuses broken A2UI references and OpenUI with unknown parts", () => {
    const head = {
      version: "1.0",
      createSurface: { surfaceId: "s", catalogId },
    };
    const update = (components: unknown[]) => [
      head,
      { version: "1.0", updateComponents: { surfaceId: "s", components } },
    ];
    expect(
      refusals(
        update([{ id: "root", component: "Stack", children: ["missing"] }]),
      ),
    ).toEqual(["format"]);
    expect(
      refusals(
        update([{ id: "root", component: "Stack", children: ["root"] }]),
      ),
    ).toEqual(["format"]);
    expect(
      refusals(
        update([
          { id: "root", component: "Stack", children: ["a", "a"] },
          { id: "a", component: "Badge", text: "x" },
        ]),
      ),
    ).toEqual(["format"]);
    expect(refusals('root = Stack([Script("x")])')).toEqual(["format"]);
    expect(refusals("root = Stack([")).toEqual(["format"]);
  });
});
