/** MCP App view of the storefront kit: bridge handshake, validated part payloads and native-only buying. */
import { act, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import {
  View,
  bridgeVersion,
  channel,
  partKey,
  payloadProps,
} from "../../src/kit/storefront/mcp-app";

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
const summary = {
  part: "product-summary",
  locale: "de-DE",
  props: {
    locale: "de-DE",
    product: {
      id: "coat",
      name: "Mantel",
      price: 249,
      currency: "EUR",
      stock: 3,
    },
  },
  links: { coat: "https://shop.example/products/coat?shop=s" },
};

describe("storefront MCP App view", () => {
  it("speaks the current bridge version", () => {
    expect(bridgeVersion).toBe("2026-01-26");
  });

  it("keeps host messages that arrive before the view subscribes", async () => {
    const target = { postMessage: () => {} } as unknown as Window;
    const { subscribe } = channel(target);
    window.dispatchEvent(
      new MessageEvent("message", {
        source: target as unknown as MessageEventSource,
        data: { jsonrpc: "2.0", id: "veyndo-init", result: {} },
      }),
    );
    const seen: unknown[] = [];
    subscribe((message) => seen.push(message.id));
    expect(seen).toEqual(["veyndo-init"]);
  });

  it("drops invalid items and unknown parts", () => {
    expect(payloadProps({ part: "nope", props: {} }).props).toEqual([]);
    const cards = payloadProps({
      part: "product-card",
      items: [
        {
          product: {
            id: "a",
            name: "A",
            url: "/p/a",
            price: 1,
            currency: "EUR",
          },
        },
        {
          product: {
            id: "b",
            name: "B",
            url: "javascript:alert(1)",
            price: 1,
            currency: "EUR",
          },
        },
      ],
    });
    expect(cards.props).toHaveLength(1);
  });

  it("renders a tool result and sends buying to the shop", async () => {
    const h = host();
    render(<View send={h.send} subscribe={h.subscribe} />);
    expect(screen.getByRole("status")).toBeInTheDocument();
    await h.deliver({
      id: "veyndo-init",
      result: { hostContext: { theme: "dark", locale: "de-DE" } },
    });
    expect(h.sent).toContainEqual({ method: "ui/notifications/initialized" });
    expect(document.documentElement.dataset.theme).toBe("dark");
    await h.deliver({
      method: "ui/notifications/tool-result",
      params: { structuredContent: {}, _meta: { [partKey]: summary } },
    });
    expect(screen.getByRole("heading", { name: "Mantel" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "In den Warenkorb" }));
    const open = h.sent.find((m) => m.method === "ui/open-link");
    expect(open?.params).toEqual({ url: summary.links.coat });
    await h.deliver({ id: 7, method: "ui/resource-teardown" });
    expect(h.sent).toContainEqual({ id: 7, result: {} });
  });

  it("shows an agent cart read-only and opens its lines in the shop", async () => {
    const h = host();
    render(<View send={h.send} subscribe={h.subscribe} />);
    await h.deliver({
      method: "ui/notifications/tool-result",
      params: {
        _meta: {
          [partKey]: {
            part: "cart-summary",
            locale: "de-DE",
            props: {
              locale: "de-DE",
              currency: "EUR",
              lines: [{ id: "coat", label: "Mantel", quantity: 2, total: 498 }],
              total: 498,
            },
            links: { coat: "https://shop.example/products/coat?shop=s" },
          },
        },
      },
    });
    expect(screen.getByText("Mantel")).toBeInTheDocument();
    // Quantity and purchase stay with the agent's native tools.
    expect(screen.queryAllByRole("button")).toHaveLength(0);
    fireEvent.click(screen.getByText("Mantel"));
    const open = h.sent.find((m) => m.method === "ui/open-link");
    expect(open?.params).toEqual({
      url: "https://shop.example/products/coat?shop=s",
    });
  });

  it("never opens non-HTTPS links and explains results it cannot show", async () => {
    const h = host();
    render(<View send={h.send} subscribe={h.subscribe} />);
    await h.deliver({
      method: "ui/notifications/tool-result",
      params: {
        _meta: {
          [partKey]: { ...summary, links: { coat: "http://evil.test" } },
        },
      },
    });
    fireEvent.click(screen.getByRole("button", { name: "In den Warenkorb" }));
    expect(h.sent.some((m) => m.method === "ui/open-link")).toBe(false);
    await h.deliver({ method: "ui/notifications/tool-result", params: {} });
    expect(screen.getByRole("status")).toHaveTextContent("cannot be shown");
  });
});
