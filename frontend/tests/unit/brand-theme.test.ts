/** Brand themes: deterministic extraction, WCAG AA contrast on every derived pair, refusal of untrusted documents and the shared Rust fixture. */
import { readFileSync, writeFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import {
  brandSiteFields,
  brandVariables,
  contrast,
  contrastFindings,
  deriveBrandTheme,
  extractPalette,
  fromOklch,
  logoShape,
  oklch,
  parseBrandTheme,
  rgb,
  suggestions,
  type PixelSample,
} from "../../src/kit/tokens";

/** A square image painted by `paint(x, y)` → [r, g, b, a]. */
function image(
  size: number,
  paint: (x: number, y: number) => number[],
  weight = 1,
): PixelSample {
  const data = new Uint8ClampedArray(size * size * 4);
  for (let y = 0; y < size; y++)
    for (let x = 0; x < size; x++) data.set(paint(x, y), (y * size + x) * 4);
  return { data, width: size, height: size, weight };
}
const white = [255, 255, 255, 255];
const orange = [240, 110, 20, 255];
const teal = [20, 150, 150, 255];
// A logo: an orange disc with a teal stripe on white.
const logo = image(48, (x, y) => {
  const inside = (x - 24) ** 2 + (y - 24) ** 2 < 20 ** 2;
  return !inside ? white : y > 27 && y < 38 ? teal : orange;
});
const square = image(32, (x, y) =>
  x > 3 && x < 28 && y > 3 && y < 28 ? [20, 20, 20, 255] : white,
);
const transparentDisc = image(32, (x, y) =>
  (x - 16) ** 2 + (y - 16) ** 2 < 14 ** 2 ? [10, 10, 10, 255] : [0, 0, 0, 0],
);

describe("contrast", () => {
  it("matches WCAG reference values", () => {
    expect(contrast("#000000", "#ffffff")).toBeCloseTo(21, 5);
    expect(contrast("#ffffff", "#ffffff")).toBe(1);
    expect(contrast("#767676", "#ffffff")).toBeCloseTo(4.54, 2);
    expect(contrast("#ffffff", "#767676")).toBe(contrast("#767676", "#ffffff"));
  });
  it("round-trips OKLCH and keeps colours inside sRGB", () => {
    for (const color of ["#0b539b", "#f06e14", "#14283d", "#808080"]) {
      const back = fromOklch(oklch(color));
      rgb(back).forEach((c, i) =>
        expect(Math.abs(c - rgb(color)[i])).toBeLessThanOrEqual(1),
      );
    }
    // Out-of-gamut chroma is reduced instead of producing invalid channels.
    expect(fromOklch({ l: 0.6, c: 0.5, h: 140 })).toMatch(/^#[0-9a-f]{6}$/);
  });
});

describe("palette extraction", () => {
  it("finds the dominant brand colour and a distinct secondary", () => {
    const palette = extractPalette([logo]);
    expect(oklch(palette.primary).h).toBeGreaterThan(30);
    expect(oklch(palette.primary).h).toBeLessThan(75);
    expect(palette.secondary).not.toBeNull();
    expect(oklch(palette.secondary!).h).toBeGreaterThan(160);
    expect(oklch(palette.neutral).c).toBeLessThan(0.03);
  });
  it("is deterministic and independent of the image order", () => {
    const mood = image(16, () => teal, 0.5);
    const a = extractPalette([logo, mood]);
    expect(extractPalette([logo, mood])).toEqual(a);
    expect(extractPalette([mood, logo])).toEqual(a);
  });
  it("treats a black-and-white logo as monochrome and ignores transparency", () => {
    const palette = extractPalette([square, transparentDisc]);
    expect(oklch(palette.primary).c).toBeLessThan(0.05);
    expect(palette.secondary).toBeNull();
    expect(extractPalette([image(4, () => [0, 0, 0, 0])]).primary).toBe(
      "#2b2b2b",
    );
    expect(suggestions(palette)).toEqual({
      pairing: "editorial",
      motion: "subtle",
    });
  });
  it("reads the logo silhouette for the radius scale", () => {
    expect(logoShape(logo)).toBe("round");
    expect(logoShape(transparentDisc)).toBe("round");
    expect(logoShape(square)).toBe("sharp");
    expect(logoShape(image(8, () => white))).toBe("soft");
    expect(logoShape({ data: [], width: 0, height: 0, weight: 1 })).toBe(
      "soft",
    );
  });
});

describe("derived themes", () => {
  // Hard cases: bright yellow, pale cyan, pure red, navy, grey and near-white brands.
  const seeds = [
    "#ffe600",
    "#a8f0ff",
    "#ff0000",
    "#0a1a5c",
    "#888888",
    "#f8f8f8",
    "#00ff66",
    "#8a2be2",
  ];
  it.each(seeds)("meet WCAG AA on every pair for %s", (primary) => {
    const theme = deriveBrandTheme({
      primary,
      secondary: null,
      neutral: "#777777",
    });
    expect(contrastFindings(theme.colors)).toEqual([]);
    expect(parseBrandTheme(theme)).toEqual({ theme });
    // The accent keeps the brand hue when the brand has one.
    if (oklch(primary).c > 0.05)
      expect(
        Math.abs(oklch(theme.colors.accent).h - oklch(primary).h) % 360,
      ).toBeLessThan(25);
  });
  it("honours vetted merchant choices and derives the site fields", () => {
    const theme = deriveBrandTheme(extractPalette([logo]), {
      pairing: "crafted",
      density: "airy",
      shape: logoShape(logo),
      source: ["logo:1", "media:2", "a", "b", "c", "d", "e"],
    });
    expect(theme.fonts).toEqual({
      pairing: "crafted",
      heading: "slab",
      body: "humanist",
    });
    expect(theme.radius).toBe("round");
    expect(theme.source).toHaveLength(6);
    expect(brandSiteFields(theme)).toMatchObject({
      font: "sans",
      radius: 24,
      spacing: 60,
    });
    expect(
      brandSiteFields({
        ...theme,
        fonts: { pairing: "timeless", heading: "old-style", body: "old-style" },
      }).font,
    ).toBe("serif");
    expect(
      brandSiteFields({
        ...theme,
        fonts: { pairing: "technical", heading: "mono", body: "mono" },
      }).font,
    ).toBe("mono");
    const vars = brandVariables(theme);
    expect(vars["--vy-color-accent-text"]).toBe(theme.colors.accentText);
    expect(vars["--vy-radius-lg"]).toBe("28px");
    expect(vars["--vy-space-4"]).toBe("20px");
    expect(vars["--veyndo-blue-text"]).toBe("var(--vy-color-accent)");
    expect(vars["--vy-font-heading"]).toContain("Rockwell");
  });
  it("refuses untrusted documents: shape, unvetted fonts and missing contrast", () => {
    const theme = deriveBrandTheme(extractPalette([logo]));
    expect(parseBrandTheme({ ...theme, css: "body{}" })).toEqual({
      error: "shape",
    });
    expect(parseBrandTheme(null)).toEqual({ error: "shape" });
    expect(
      parseBrandTheme({ ...theme, fonts: { ...theme.fonts, heading: "mono" } }),
    ).toEqual({ error: "fonts" });
    expect(
      parseBrandTheme({
        ...theme,
        colors: { ...theme.colors, muted: theme.colors.surface },
      }),
    ).toEqual({ error: "contrast" });
    expect(
      parseBrandTheme({
        ...theme,
        colors: { ...theme.colors, accent: "#ABCDEF" },
      }),
    ).toEqual({ error: "shape" });
  });
});

// The Rust validator reads the same derived documents; tests run from frontend/.
const fixtureFile =
  "../extensions/apps/site-builder/tests/brand-theme-fixture.json";
const fixture = () => ({
  vivid: deriveBrandTheme(extractPalette([logo]), {
    shape: logoShape(logo),
    source: ["logo:fixture"],
  }),
  monochrome: deriveBrandTheme(extractPalette([square]), {
    shape: logoShape(square),
  }),
  pastel: deriveBrandTheme(
    { primary: "#a8f0ff", secondary: "#ffd6e8", neutral: "#8a8f99" },
    { density: "compact", pairing: "timeless" },
  ),
});
describe("shared fixture", () => {
  it("matches the derivation (WRITE_BRAND_FIXTURE=1 regenerates)", () => {
    if (process.env.WRITE_BRAND_FIXTURE)
      writeFileSync(fixtureFile, `${JSON.stringify(fixture(), null, 2)}\n`);
    expect(JSON.parse(readFileSync(fixtureFile, "utf8"))).toEqual(fixture());
  });
});
