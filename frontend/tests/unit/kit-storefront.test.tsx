/** kit/storefront contracts: one schema per part validates props, yields JSON Schemas, web elements and builder forms. */
import { readFileSync, readdirSync } from "node:fs";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import {
  PartForm,
  accountOverview,
  addToCartEvent,
  cartSummary,
  checkoutEvent,
  checkoutSteps,
  orderList,
  partById,
  parts,
  productCard,
  productSummary,
  registerPartElements,
} from "../../src/kit/storefront";

const product = {
  id: "p-1",
  name: "Linen shirt",
  url: "/p/linen-shirt",
  price: 49.9,
  currency: "EUR",
  stock: 3,
};

describe("part contracts", () => {
  it("names every part once and derives its element", () => {
    expect(new Set(parts.map((p) => p.id)).size).toBe(parts.length);
    for (const part of parts) {
      expect(part.element).toBe(`veyndo-${part.id}`);
      expect(partById(part.id)).toBe(part);
    }
    expect(partById("missing")).toBeUndefined();
  });

  it("validates untrusted props and fills defaults", () => {
    const ok = productCard.parse({ product });
    expect(ok).toMatchObject({
      ok: true,
      props: { locale: "en-GB", priceKind: "gross" },
    });
    const bad = productCard.parse({
      product: { ...product, url: "javascript:alert(1)", currency: "eur" },
    });
    expect(bad.ok).toBe(false);
    if (!bad.ok) {
      expect(bad.issues.join(" ")).toContain("product.url");
      expect(bad.issues.join(" ")).toContain("product.currency");
    }
    expect(
      productCard.parse({ product: { ...product, url: "//evil" } }).ok,
    ).toBe(false);
    expect(productCard.parse(null).ok).toBe(false);
  });

  it("validates builder configurations separately from props", () => {
    expect(productCard.parseConfig({ productId: "p-1" })).toEqual({
      ok: true,
      config: { productId: "p-1", priceKind: "gross" },
    });
    expect(productCard.parseConfig({}).ok).toBe(false);
    expect(accountOverview.parseConfig({ recent: 99 }).ok).toBe(false);
    expect(checkoutSteps.parseConfig({})).toEqual({
      ok: true,
      config: { current: "details" },
    });
  });

  it("publishes JSON Schemas with patterns, formats and defaults", () => {
    for (const part of parts) {
      expect(part.jsonSchema()).toMatchObject({ type: "object" });
      expect(part.configSchema()).toMatchObject({ type: "object" });
    }
    const props = productCard.jsonSchema() as any;
    expect(props.$schema).toContain("2020-12");
    expect(props.properties.product.properties.url.pattern).toBeTruthy();
    const config = productCard.configSchema() as any;
    expect(config.properties.productId.format).toBe("product-id");
    expect(config.required).toEqual(["productId"]);
    expect(config.properties.priceKind.enum).toEqual(["gross", "net"]);
  });
});

describe("part components", () => {
  it("renders a product card in the shop language", () => {
    const P = productCard.Component;
    render(<P locale="de-DE" product={product} priceKind="gross" />);
    for (const link of screen.getAllByRole("link"))
      expect(link).toHaveAttribute("href", "/p/linen-shirt");
    expect(
      screen.getByRole("heading", { name: "Linen shirt" }),
    ).toBeInTheDocument();
    expect(screen.getByText("inkl. MwSt.")).toBeInTheDocument();
    expect(screen.getByText(/49,90/)).toBeInTheDocument();
  });

  it("asks the host to add the chosen quantity", () => {
    const onAddToCart = vi.fn();
    const P = productSummary.Component;
    const props = productSummary.parse({
      locale: "en-GB",
      product: { ...product, purchaseSteps: 1, minPurchase: 1 },
    });
    if (!props.ok) throw Error(props.issues.join());
    render(<P {...props.props} onAddToCart={onAddToCart} />);
    fireEvent.change(screen.getByRole("spinbutton", { name: "Quantity" }), {
      target: { value: "2" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Add to cart" }));
    expect(onAddToCart).toHaveBeenCalledWith({ productId: "p-1", quantity: 2 });
  });

  it("offers variants, quantity prices, delivery and a host-controlled quantity", () => {
    const P = productSummary.Component;
    const parsed = productSummary.parse({
      locale: "de-DE",
      product: {
        ...product,
        kicker: "Brand / Coats",
        sku: "C-1",
        rating: { average: 4, count: 2 },
      },
      shippingExtra: true,
      groups: [
        {
          id: "size",
          label: "Größe",
          selected: "M",
          options: [
            { value: "m", label: "M", available: true, selected: true },
            { value: "xl", label: "XL", available: false, selected: false },
            { value: "l", label: "L", available: true, selected: false },
          ],
        },
      ],
      moreVariants: "more",
      showRules: true,
      tiers: [
        { quantity: 1, unitPrice: 49.9, discountPercent: 0 },
        { quantity: 10, unitPrice: 44.9, discountPercent: 10 },
      ],
      selectedTier: 1,
      delivery: {
        minDays: 2,
        maxDays: 4,
        method: "Standard",
        country: "Deutschland",
      },
      quantity: 3,
    });
    if (!parsed.ok) throw Error(parsed.issues.join());
    const onSelect = vi.fn(),
      onOpen = vi.fn(),
      onQuantity = vi.fn();
    render(
      <P
        {...parsed.props}
        onSelect={onSelect}
        onOpen={onOpen}
        onQuantity={onQuantity}
        slot={(name) => <p>slot {name}</p>}
      />,
    );
    expect(screen.getByRole("button", { name: "XL" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "L" }));
    expect(onSelect).toHaveBeenCalledWith({ group: "size", value: "l" });
    fireEvent.click(screen.getByRole("button", { name: /Bewertungen/ }));
    fireEvent.click(
      screen.getByRole("button", { name: "Weitere Varianten laden" }),
    );
    expect(onOpen.mock.calls.map((c) => c[0].id)).toEqual([
      "reviews",
      "variants",
    ]);
    const input = screen.getByRole("spinbutton");
    expect(input).toHaveValue(3);
    fireEvent.change(input, { target: { value: "5" } });
    expect(onQuantity).toHaveBeenLastCalledWith({
      lineId: "p-1",
      quantity: 5,
      settled: false,
    });
    fireEvent.blur(input);
    expect(onQuantity).toHaveBeenLastCalledWith({
      lineId: "p-1",
      quantity: 3,
      settled: true,
    });
    expect(screen.getByText("Mengenpreise")).toBeInTheDocument();
    expect(screen.getByText("2–4 Kalendertage · Standard")).toBeInTheDocument();
    expect(screen.getByText("slot beforeBuy")).toBeInTheDocument();
    expect(screen.getByText("SKU C-1")).toBeInTheDocument();
  });

  it("disables buying when sold out", () => {
    const P = productSummary.Component;
    const props = productSummary.parse({ product: { ...product, stock: 0 } });
    if (!props.ok) throw Error(props.issues.join());
    render(<P {...props.props} />);
    expect(screen.getByText("Sold out")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Add to cart" })).toBeDisabled();
  });

  it("summarizes a cart and offers checkout only with a handler", () => {
    const P = cartSummary.Component;
    const parsed = cartSummary.parse({
      locale: "es-ES",
      currency: "EUR",
      lines: [{ id: "l1", label: "Linen shirt", quantity: 2, total: 99.8 }],
      subtotal: 99.8,
      shipping: 4.9,
      total: 104.7,
    });
    if (!parsed.ok) throw Error(parsed.issues.join());
    const cart = parsed.props;
    const { rerender } = render(<P {...cart} />);
    expect(
      screen.getByRole("region", { name: "Tu pedido" }),
    ).toBeInTheDocument();
    expect(screen.queryByRole("button")).toBeNull();
    const onCheckout = vi.fn();
    rerender(<P {...cart} onCheckout={onCheckout} />);
    fireEvent.click(screen.getByRole("button", { name: "Ir a la caja" }));
    expect(onCheckout).toHaveBeenCalledOnce();
    rerender(<P {...cart} lines={[]} onCheckout={onCheckout} />);
    expect(screen.getByText("Tu carrito está vacío.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Ir a la caja" })).toBeNull();
  });

  it("changes quantities within the purchase rules and shows host additions", () => {
    const P = cartSummary.Component;
    const parsed = cartSummary.parse({
      locale: "en-GB",
      currency: "EUR",
      lines: [
        {
          id: "l1",
          label: "Socks",
          quantity: 2,
          unitPrice: 5,
          total: 10,
          minPurchase: 2,
          purchaseSteps: 2,
          maxPurchase: 4,
        },
      ],
      discounts: [{ id: "d", label: "", amount: 1 }],
      tax: 1.6,
      total: 9,
      delivery: { earliest: "2026-12-01", latest: "2026-12-03" },
    });
    if (!parsed.ok) throw Error(parsed.issues.join());
    const onQuantity = vi.fn();
    render(
      <P
        {...parsed.props}
        onQuantity={onQuantity}
        renderLineDetail={(id) => <em>detail {id}</em>}
        slot={(name) => <aside>{name}</aside>}
      />,
    );
    expect(
      screen.getByRole("button", { name: "Quantity − Socks" }),
    ).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Quantity + Socks" }));
    expect(onQuantity).toHaveBeenLastCalledWith({ lineId: "l1", quantity: 4 });
    fireEvent.click(screen.getByRole("button", { name: "Remove" }));
    expect(onQuantity).toHaveBeenLastCalledWith({ lineId: "l1", quantity: 0 });
    expect(screen.getByText("detail l1")).toBeInTheDocument();
    expect(screen.getByText("beforeTotals")).toBeInTheDocument();
    expect(screen.getByText("Discount")).toBeInTheDocument();
    expect(screen.getByText("VAT")).toBeInTheDocument();
    expect(
      screen.getByText("Delivery: 2026-12-01 — 2026-12-03"),
    ).toBeInTheDocument();
  });

  it("marks the current checkout step", () => {
    const P = checkoutSteps.Component;
    render(<P locale="en-GB" current="review" />);
    const current = screen
      .getAllByRole("listitem")
      .filter((item) => item.getAttribute("aria-current") === "step");
    expect(current).toHaveLength(1);
  });

  it("shows the account home and opens orders and pages", () => {
    const P = accountOverview.Component;
    const parsed = accountOverview.parse({
      locale: "de-DE",
      name: "Ada",
      counts: { orders: 3, addresses: 1, downloads: 0 },
      orders: [
        {
          id: "o1",
          number: "10001",
          date: "2026-11-01",
          state: "Bezahlt",
          total: 20,
          currency: "EUR",
          items: [{ label: "Mantel", quantity: 1 }],
        },
        {
          id: "o2",
          number: "10002",
          state: "Offen",
          total: 5,
          currency: "EUR",
        },
        {
          id: "o3",
          number: "10003",
          state: "Offen",
          total: 5,
          currency: "EUR",
        },
      ],
    });
    if (!parsed.ok) throw Error(parsed.issues.join());
    const onOpen = vi.fn();
    render(
      <P {...parsed.props} onOpen={onOpen} slot={(name) => <p>{name}</p>} />,
    );
    expect(screen.getByRole("heading", { name: "Ada" })).toBeInTheDocument();
    expect(screen.getByText("1. November 2026")).toBeInTheDocument();
    expect(screen.queryByText("10003")).toBeNull(); // two recent orders
    fireEvent.click(
      screen.getByRole("button", { name: "Bestellung ansehen 10001" }),
    );
    expect(onOpen).toHaveBeenLastCalledWith({ kind: "order", id: "o1" });
    fireEvent.click(screen.getByRole("button", { name: /Adressen/ }));
    expect(onOpen).toHaveBeenLastCalledWith({ kind: "page", id: "addresses" });
    expect(screen.getByText("end")).toBeInTheDocument();
  });

  it("invites a first purchase when there are no orders", () => {
    const P = orderList.Component;
    const onOpen = vi.fn();
    render(<P locale="en-GB" orders={[]} onOpen={onOpen} />);
    fireEvent.click(screen.getByRole("button", { name: "Continue shopping" }));
    expect(onOpen).toHaveBeenCalledWith({ kind: "page", id: "shop" });
  });

  it("styles every kit class its components use", () => {
    const css = readFileSync("src/kit/storefront/storefront.css", "utf8");
    const used = new Set<string>();
    for (const file of readdirSync("src/kit/storefront").filter((f) =>
      f.endsWith(".tsx"),
    ))
      for (const match of readFileSync(
        `src/kit/storefront/${file}`,
        "utf8",
      ).matchAll(/className="([^"]+)"/g))
        match[1]
          .split(/\s+/)
          .filter((c) => c.startsWith("vy-sf-"))
          .forEach((c) => used.add(c));
    expect(used.size).toBeGreaterThan(20);
    for (const name of used)
      expect(css, name).toMatch(new RegExp(`\\.${name}(?![\\w-])`));
  });
});

describe("web elements", () => {
  it("upgrades valid props and leaves invalid fallbacks in place", async () => {
    registerPartElements(parts);
    registerPartElements(parts); // idempotent
    const host = document.createElement("div");
    document.body.append(host);
    const listener = vi.fn();
    host.addEventListener(addToCartEvent, (event) =>
      listener((event as CustomEvent).detail),
    );
    const checkout = vi.fn();
    host.addEventListener(checkoutEvent, checkout);

    const element = document.createElement(productSummary.element);
    element.setAttribute("locale", "de-DE");
    element.setAttribute("props", JSON.stringify({ product }));
    await act(async () => host.append(element));
    fireEvent.click(element.querySelector("button")!);
    expect(listener).toHaveBeenCalledWith({ productId: "p-1", quantity: 1 });
    expect(element.textContent).toContain("In den Warenkorb");

    const broken = document.createElement(productCard.element);
    broken.innerHTML = "<a href='/p/x'>Server markup</a>";
    broken.setAttribute("props", "{not json");
    await act(async () => host.append(broken));
    expect(broken.dataset.invalid).toBe("true");
    expect(broken.textContent).toBe("Server markup");

    const cart = document.createElement(cartSummary.element);
    cart.setAttribute(
      "props",
      JSON.stringify({
        currency: "EUR",
        lines: [{ id: "l", label: "A", quantity: 1, total: 1 }],
        subtotal: 1,
        total: 1,
      }),
    );
    await act(async () => host.append(cart));
    fireEvent.click(
      [...cart.querySelectorAll("button")].find(
        (b) => b.textContent === "Go to checkout",
      )!,
    );
    expect(checkout).toHaveBeenCalledOnce();
    await act(async () => host.remove());
  });
});

describe("builder form", () => {
  it("renders fields from the configuration schema", () => {
    const onChange = vi.fn();
    const { rerender } = render(
      <PartForm
        part={productCard}
        value={{ productId: "p-1" }}
        onChange={onChange}
        label={(field) => `L:${field}`}
      />,
    );
    fireEvent.change(screen.getByLabelText(/L:priceKind/), {
      target: { value: "net" },
    });
    expect(onChange).toHaveBeenLastCalledWith({
      productId: "p-1",
      priceKind: "net",
    });
    rerender(
      <PartForm
        part={productCard}
        value={{}}
        onChange={onChange}
        label={(field) => field}
        renderFormat={(format, field) => (
          <span>
            picker {format} {field}
          </span>
        )}
      />,
    );
    expect(screen.getByText("picker product-id productId")).toBeInTheDocument();

    rerender(
      <PartForm
        part={cartSummary}
        value={{}}
        onChange={onChange}
        label={(field) => field}
      />,
    );
    expect(
      screen.getByRole("checkbox", { name: "showCheckout" }),
    ).toBeChecked();
    rerender(
      <PartForm
        part={accountOverview}
        value={{}}
        onChange={onChange}
        label={(field) => field}
      />,
    );
    fireEvent.change(screen.getByRole("spinbutton", { name: /recent/ }), {
      target: { value: "7" },
    });
    expect(onChange).toHaveBeenLastCalledWith({ recent: 7 });
  });
});
