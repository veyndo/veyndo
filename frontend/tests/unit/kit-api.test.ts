/** kit/api: route keys and path parameters are checked at compile time, parameters cannot escape, operations unwrap MCP results. */
import { describe, expect, it, vi } from "vitest";
import {
  createApiClient,
  fillPath,
  OperationError,
  routes,
  type OperationResult,
  type PathParams,
} from "../../src/kit/api";
import { routeSources } from "../../src/kit/api/generated/routes";

describe("route catalogue", () => {
  it("splits every generated key into method, path and source", () => {
    expect(routes).toHaveLength(Object.keys(routeSources).length);
    const product = routes.find(
      (route) => route.key === "GET /api/merchant/products/{id}",
    )!;
    expect(product).toMatchObject({
      method: "GET",
      path: "/api/merchant/products/{id}",
    });
    expect(product.source).toMatch(/\.rs$/);
    expect(
      routes.every((route) => route.key === `${route.method} ${route.path}`),
    ).toBe(true);
  });

  it("encodes parameters and refuses values that leave their segment", () => {
    expect(fillPath("/a/{id}/b/{part}", { id: " x y ", part: 3 })).toBe(
      "/a/x%20y/b/3",
    );
    for (const bad of [
      "",
      " ",
      ".",
      "..",
      "a/b",
      "a\\b",
      "a?b",
      "a#b",
      undefined,
    ])
      expect(() => fillPath("/a/{id}", { id: bad })).toThrow(
        "Invalid parameter: id",
      );
  });

  it("derives path parameter types from the template", () => {
    const params: PathParams<"/api/pos/{channel}/{operation}"> = {
      channel: "c",
      operation: "o",
    };
    // @ts-expect-error a path parameter is missing
    const missing: PathParams<"/api/pos/{channel}/{operation}"> = {
      channel: "c",
    };
    expect([params, missing]).toHaveLength(2);
  });
});

describe("client", () => {
  it("calls routes with filled parameters, query and body", async () => {
    const transport = vi.fn().mockResolvedValue({ ok: true });
    const api = createApiClient(transport);
    await api.route("GET /api/merchant/products");
    await api.route("GET /api/merchant/products/{id}", {
      params: { id: "coat" },
      query: { language: "de-DE", limit: 5, skip: undefined },
    });
    await api.route("POST /api/agent/plan", { body: { goal: "x" } });
    expect(transport.mock.calls).toEqual([
      ["/api/merchant/products", undefined, "GET"],
      ["/api/merchant/products/coat?language=de-DE&limit=5", undefined, "GET"],
      ["/api/agent/plan", { goal: "x" }, "POST"],
    ]);
    // @ts-expect-error unknown route key
    await api.route("GET /api/not-a-route").catch(() => {});
    // @ts-expect-error path parameters are required
    await api.route("GET /api/merchant/products/{id}").catch(() => {});
  });

  it("calls operations over MCP and returns their structured result", async () => {
    const transport = vi.fn().mockResolvedValue({
      result: { structuredContent: { revision: 3, saved: false, valid: true } },
    });
    const api = createApiClient(transport);
    const result = await api.call("automation.validate_flow", {} as never);
    const typed: OperationResult<"automation.validate_flow"> = result;
    expect(typed.revision).toBe(3);
    const [path, body, method] = transport.mock.calls[0];
    expect([path, method]).toEqual(["/mcp", "POST"]);
    expect(body).toMatchObject({
      jsonrpc: "2.0",
      method: "tools/call",
      params: { name: "automation.validate_flow", arguments: {} },
    });
    await api.call("currency.select", { currency: "EUR", revision: 1 });
    // @ts-expect-error the input contract requires a revision
    await api.call("currency.select", { currency: "EUR" });
    // @ts-expect-error unknown operation
    await api.call("currency.unknown", {}).catch(() => {});
  });

  it("turns tool errors and protocol errors into OperationError", async () => {
    const tool = createApiClient(
      vi.fn().mockResolvedValue({
        result: {
          isError: true,
          content: [{ type: "text", text: "Revision changed" }],
        },
      }),
    );
    await expect(
      tool.call("currency.select", { currency: "EUR", revision: 1 }),
    ).rejects.toMatchObject({
      name: "OperationError",
      message: "Revision changed",
      operation: "currency.select",
    });
    const protocol = createApiClient(
      vi
        .fn()
        .mockResolvedValue({
          error: { code: -32601, message: "Unknown tool" },
        }),
    );
    const failure = await protocol
      .call("currency.list", null)
      .catch((error: unknown) => error);
    expect(failure).toBeInstanceOf(OperationError);
    expect(failure).toMatchObject({ code: -32601, message: "Unknown tool" });
    const empty = createApiClient(vi.fn().mockResolvedValue({}));
    await expect(empty.call("currency.list", null)).rejects.toThrow(
      "Operation failed",
    );
    const silent = createApiClient(
      vi.fn().mockResolvedValue({ result: { isError: true } }),
    );
    await expect(silent.call("currency.list", null)).rejects.toThrow(
      "Operation failed",
    );
    const unnamed = createApiClient(vi.fn().mockResolvedValue({ error: {} }));
    await expect(unnamed.call("currency.list", null)).rejects.toThrow(
      "Operation failed",
    );
    const plain = createApiClient(vi.fn().mockResolvedValue({ result: {} }));
    expect(await plain.call("currency.list", null)).toBeNull();
  });
});
