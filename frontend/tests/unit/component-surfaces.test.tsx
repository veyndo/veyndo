/** Every surface renders catalog documents through the one kit renderer: MCP App view, storefront element, app documents in the conversation and Studio. */
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { View } from "../../src/kit/storefront/mcp-app";
import {
  actionEvent,
  catalogId,
  documentKey,
  registerDocumentElement,
  renderMcpDocument,
} from "../../src/kit/catalog";
import ConversationAppViews from "../../src/shared/apps/ConversationAppViews";
import { LocaleProvider } from "../../src/shared/i18n/i18n";

type Message = Record<string, unknown>;
function host() {
  const sent: Message[] = [];
  let listener: (message: Message) => void = () => {};
  return {
    sent,
    send: (message: Message) => sent.push(message),
    subscribe: (next: (message: Message) => void) => {
      listener = next;
      return () => {};
    },
    deliver: (message: Message) =>
      act(() => listener({ jsonrpc: "2.0", ...message })),
  };
}
const tree = (root: unknown) => ({
  format: "veyndo-ui/1",
  catalog: catalogId,
  root,
});
const product = {
  id: "coat",
  name: "Mantel",
  url: "/products/coat",
  price: 249,
  currency: "EUR",
  stock: 3,
};

describe("MCP App view", () => {
  it("renders a document of the tool result on the mcp-app surface and opens links through the host", async () => {
    const h = host();
    render(
      <View
        send={h.send}
        subscribe={h.subscribe}
        renderMeta={renderMcpDocument}
      />,
    );
    await h.deliver({
      id: "veyndo-init",
      result: { hostContext: { locale: "de-DE" } },
    });
    await h.deliver({
      method: "ui/notifications/tool-result",
      params: {
        structuredContent: {},
        _meta: {
          [documentKey]: {
            locale: "de-DE",
            origins: ["https://shop.example"],
            data: { product },
            document: tree({
              component: "Stack",
              children: [
                {
                  component: "ProductCard",
                  props: { product: { path: "/product" } },
                },
                {
                  component: "LinkButton",
                  props: { label: "Alle Mäntel", href: "/coats" },
                },
                { component: "AccountOverview", props: {} },
              ],
            }),
          },
        },
      },
    });
    expect(screen.getByText("Mantel")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("link", { name: "Alle Mäntel" }));
    expect(
      h.sent.find((message) => message.method === "ui/open-link")?.params,
    ).toEqual({
      url: "https://shop.example/coats",
    });
    // Personal account data is not admitted on the mcp-app surface.
    expect(
      document.querySelector('[data-refusal="not-admitted"]'),
    ).not.toBeNull();
  });

  it("leaves results without a document to the part payload", async () => {
    const h = host();
    render(
      <View
        send={h.send}
        subscribe={h.subscribe}
        renderMeta={renderMcpDocument}
      />,
    );
    await h.deliver({
      method: "ui/notifications/tool-result",
      params: { structuredContent: {}, _meta: {} },
    });
    expect(screen.getByRole("status")).toBeInTheDocument();
  });
});

describe("storefront element", () => {
  it("renders the document with the data and actions the page admits and reports actions as DOM events", async () => {
    registerDocumentElement();
    registerDocumentElement();
    const element = document.createElement("veyndo-document");
    element.setAttribute("locale", "es-ES");
    element.setAttribute("actions", "add");
    element.setAttribute("data", JSON.stringify({ product }));
    element.setAttribute(
      "document",
      JSON.stringify(
        tree({
          component: "Stack",
          children: [
            {
              component: "ProductCard",
              props: { product: { path: "/product" } },
            },
            {
              component: "Button",
              props: { label: "Añadir" },
              action: { name: "add", context: { id: "coat" } },
            },
          ],
        }),
      ),
    );
    const heard = vi.fn();
    document.body.addEventListener(actionEvent, (event) =>
      heard((event as CustomEvent).detail),
    );
    await act(async () => {
      document.body.append(element);
    });
    expect(screen.getByText("Mantel")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Añadir" }));
    expect(heard).toHaveBeenCalledWith(
      expect.objectContaining({
        name: "add",
        context: { id: "coat" },
        event: "press",
      }),
    );
    await act(async () => {
      element.setAttribute("document", 'root = Stack([Badge("<b>x</b>")])');
    });
    expect(
      element.querySelector('[data-refusal="unsafe-html"]'),
    ).not.toBeNull();
    expect(element.textContent).toContain("Esta parte no se puede mostrar");
    await act(async () => {
      element.setAttribute("document", "{broken");
    });
    expect(element.querySelector('[data-refusal="format"]')).not.toBeNull();
    element.remove();
  });
});

describe("app documents in the conversation", () => {
  it("render in place on the app surface, bound only to the read the host ran", async () => {
    const uri = "ui://veyndo/shop/guide/view/digest";
    const definition = {
      label: { en: "Care guide" },
      view: {},
      toolName: "app.guide.list",
      localeChain: ["en"],
      document: tree({
        component: "Card",
        props: { title: { path: "/result/elements/0/title" } },
        children: [
          {
            component: "Text",
            props: { text: { path: "/result/elements/0/body" } },
          },
          {
            component: "Text",
            props: { text: { path: "/result/elements/1/body" } },
          },
          {
            component: "ProductCard",
            props: { product: { path: "/result/elements/0" } },
          },
        ],
      }),
    };
    const request = vi.fn(async () => ({
      result: {
        contents: [
          {
            uri,
            mimeType: "text/html;profile=mcp-app",
            _meta: { "ai.veyndo/definition": definition },
          },
        ],
      },
    }));
    const { container } = render(
      <LocaleProvider>
        <ConversationAppViews
          request={request}
          views={[
            {
              resourceUri: uri,
              arguments: { product: "coat" },
              result: {
                structuredContent: {
                  elements: [
                    { title: "Wool care", body: "Air it after wearing." },
                  ],
                },
              },
            },
          ]}
        />
      </LocaleProvider>,
    );
    await waitFor(() =>
      expect(
        screen.getByRole("heading", { name: "Wool care" }),
      ).toBeInTheDocument(),
    );
    expect(screen.getByText("Air it after wearing.")).toBeInTheDocument();
    expect(
      screen.getByRole("region", { name: "Care guide" }),
    ).toBeInTheDocument();
    expect(container.querySelector("iframe")).toBeNull();
    // The second record was not in the result, and apps cannot render commerce patterns.
    expect(
      container.querySelector('[data-refusal="data-not-admitted"]'),
    ).not.toBeNull();
    expect(
      container.querySelector('[data-refusal="not-admitted"]'),
    ).not.toBeNull();
    expect(request).toHaveBeenCalledTimes(1);
  });
});
