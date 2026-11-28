/** Motion language: generated intensity tokens, reduced motion, scroll reveal and page entrance, and a storefront rendered at each intensity. */
import { readFileSync } from "node:fs";
import { act, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  motionAllowed,
  motionDuration,
  motionIntensities,
  motionIntensity,
  observeReveal,
  playPageEnter,
  tokens,
} from "../../src/kit/tokens";
import { productCard } from "../../src/kit/storefront";

/** IntersectionObserver stand-in that lets the test decide which elements enter the viewport. */
class Viewport {
  static last: Viewport;
  observed = new Set<Element>();
  constructor(public notify: IntersectionObserverCallback) {
    Viewport.last = this;
  }
  observe(el: Element) {
    this.observed.add(el);
  }
  unobserve(el: Element) {
    this.observed.delete(el);
  }
  disconnect() {
    this.observed.clear();
  }
  enter(el: Element) {
    this.notify(
      [{ target: el, isIntersecting: true } as IntersectionObserverEntry],
      this as unknown as IntersectionObserver,
    );
  }
}
const reducedMotion = (matches: boolean) =>
  vi.stubGlobal(
    "matchMedia",
    (query: string) =>
      ({ matches: matches && query.includes("reduce") }) as MediaQueryList,
  );
afterEach(() => vi.unstubAllGlobals());

describe("generated motion tokens", () => {
  const css = readFileSync("src/kit/tokens/tokens.css", "utf8");
  it("defines every intensity and makes reduced motion win over all of them", () => {
    for (const level of motionIntensities) {
      expect(css).toContain(`[data-motion="${level}"] {`);
      for (const key of Object.keys(tokens["motion-intensity"][level]))
        expect(css).toContain(`--vy-motion-${key}:`);
    }
    const reduced = css.slice(css.indexOf("prefers-reduced-motion"));
    expect(reduced).toContain("[data-motion] {");
    expect(reduced).toContain("--vy-motion-reveal: 0ms;");
    expect(reduced).toContain("--vy-motion-distance: 0px;");
    expect(reduced).toContain("--vy-motion-press: 1;");
    // Off is still and expressive moves further than subtle.
    const intensity = tokens["motion-intensity"];
    expect(intensity.off.page).toBe("0ms");
    expect(parseFloat(intensity.expressive.distance)).toBeGreaterThan(
      parseFloat(intensity.subtle.distance),
    );
  });
  it("keeps primitives on tokens only", () => {
    const primitives = readFileSync("src/kit/tokens/motion.css", "utf8");
    expect(primitives).not.toMatch(/\d+ms|#[0-9a-f]{3,6}\b/i);
    expect(primitives).toContain("[data-motion] .vy-enter");
  });
});

describe("motion runtime", () => {
  it("reads intensities defensively and durations in milliseconds", () => {
    expect(motionIntensity("expressive")).toBe("expressive");
    expect(motionIntensity("wild")).toBe("off");
    expect(motionIntensity(undefined)).toBe("off");
    const el = document.createElement("div");
    el.style.setProperty("--vy-motion-page", "0.3s");
    document.body.append(el);
    expect(motionDuration(el, "--vy-motion-page")).toBe(300);
    el.style.setProperty("--vy-motion-page", "120ms");
    expect(motionDuration(el, "--vy-motion-page")).toBe(120);
    expect(motionDuration(el, "--vy-missing")).toBe(0);
    el.remove();
  });
  it("needs an explicit intensity and no reduced-motion preference", () => {
    reducedMotion(false);
    const shop = document.createElement("div");
    const child = shop.appendChild(document.createElement("p"));
    expect(motionAllowed(child)).toBe(false);
    shop.dataset.motion = "subtle";
    expect(motionAllowed(child)).toBe(true);
    shop.dataset.motion = "off";
    expect(motionAllowed(child)).toBe(false);
    shop.dataset.motion = "expressive";
    reducedMotion(true);
    expect(motionAllowed(child)).toBe(false);
  });
  it("plays the page entrance only when motion is allowed and timed", () => {
    reducedMotion(false);
    const shop = document.createElement("div");
    const main = shop.appendChild(document.createElement("main"));
    const animate = vi.fn(() => ({}) as Animation);
    main.animate = animate;
    document.body.append(shop);
    expect(playPageEnter(shop)).toBeNull();
    shop.dataset.motion = "expressive";
    shop.style.setProperty("--vy-motion-page", "380ms");
    shop.style.setProperty("--vy-motion-distance", "28px");
    playPageEnter(shop);
    expect(animate).toHaveBeenCalledWith(
      [
        { opacity: 0, transform: "translateY(28px)" },
        { opacity: 1, transform: "none" },
      ],
      expect.objectContaining({ duration: 380 }),
    );
    shop.style.setProperty("--vy-motion-page", "0ms");
    expect(playPageEnter(shop)).toBeNull();
    shop.remove();
  });
  it("keeps content visible where IntersectionObserver is missing", () => {
    const shop = document.createElement("div");
    expect(observeReveal(shop)).toBeTypeOf("function");
  });
});

describe("storefront at each intensity", () => {
  const P = productCard.Component;
  const product = {
    id: "p",
    name: "Linen shirt",
    url: "/p/linen",
    price: 49,
    currency: "EUR",
  };
  const shop = (motion: string | undefined) =>
    render(
      <div className="shop" data-motion={motion}>
        <section className="shop-hero vy-enter">Hero</section>
        {[1, 2].map((i) => (
          <div key={i} className="shop-tile" data-vy-reveal>
            <P
              locale="en-GB"
              product={{ ...product, id: `p${i}` }}
              priceKind="gross"
            />
          </div>
        ))}
      </div>,
    ).container.firstElementChild as HTMLElement;

  it.each([
    ["off", "shown"],
    ["subtle", "pending"],
    ["expressive", "pending"],
    [undefined, "shown"],
  ])("intensity %s reveals tiles as %s first", async (motion, first) => {
    vi.stubGlobal("IntersectionObserver", Viewport);
    reducedMotion(false);
    const root = shop(motion);
    const stop = observeReveal(root);
    const tiles = [...root.querySelectorAll(".shop-tile")];
    expect(root.querySelectorAll(".vy-sf-card")).toHaveLength(2);
    expect(tiles.map((t) => t.getAttribute("data-vy-reveal"))).toEqual([
      first,
      first,
    ]);
    if (first === "pending") {
      act(() => Viewport.last.enter(tiles[0]));
      expect(tiles[0].getAttribute("data-vy-reveal")).toBe("shown");
      expect(tiles[1].getAttribute("data-vy-reveal")).toBe("pending");
      // Tiles rendered later are observed too.
      const late = root.appendChild(document.createElement("div"));
      late.setAttribute("data-vy-reveal", "");
      await act(async () => {});
      expect(late.getAttribute("data-vy-reveal")).toBe("pending");
    }
    stop();
    // Cleanup never leaves content hidden.
    expect(root.querySelector('[data-vy-reveal="pending"]')).toBeNull();
  });
  it("shows everything at once under reduced motion, even when expressive", () => {
    vi.stubGlobal("IntersectionObserver", Viewport);
    reducedMotion(true);
    const root = shop("expressive");
    observeReveal(root)();
    for (const tile of root.querySelectorAll(".shop-tile"))
      expect(tile.getAttribute("data-vy-reveal")).toBe("shown");
  });
});
