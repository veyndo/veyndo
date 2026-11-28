/** Brand themes: deterministic palette extraction, WCAG AA contrast adjustment, vetted fonts and scales, and the typed token document. */
import { z } from "zod";

/** A colour as lowercase `#rrggbb`. */
export type Hex = string;
const hexPattern = /^#[0-9a-f]{6}$/;

// ---------------------------------------------------------------- colour math

const channel = (c: number) =>
  c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
const gamma = (c: number) =>
  c <= 0.0031308 ? 12.92 * c : 1.055 * c ** (1 / 2.4) - 0.055;

/** The sRGB channels of a hex colour, 0..255. */
export function rgb(hex: Hex): [number, number, number] {
  const n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}
/** A hex colour from sRGB channels, clamped and rounded. */
export function hex([r, g, b]: [number, number, number]): Hex {
  return (
    "#" +
    [r, g, b]
      .map((c) =>
        Math.round(Math.min(255, Math.max(0, c)))
          .toString(16)
          .padStart(2, "0"),
      )
      .join("")
  );
}
/** WCAG 2.x relative luminance. */
export function luminance(color: Hex): number {
  const [r, g, b] = rgb(color).map((c) => channel(c / 255));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}
/** WCAG 2.x contrast ratio, 1..21. */
export function contrast(a: Hex, b: Hex): number {
  const [x, y] = [luminance(a), luminance(b)].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
}

/** OKLCH: perceptual lightness 0..1, chroma 0..~0.37, hue in degrees. */
export type Oklch = { l: number; c: number; h: number };
function toOklab([r, g, b]: [number, number, number]) {
  const [lr, lg, lb] = [r, g, b].map((v) => channel(v / 255));
  const l = Math.cbrt(
    0.4122214708 * lr + 0.5363325363 * lg + 0.0514459929 * lb,
  );
  const m = Math.cbrt(
    0.2119034982 * lr + 0.6806995451 * lg + 0.1073969566 * lb,
  );
  const s = Math.cbrt(
    0.0883024619 * lr + 0.2817188376 * lg + 0.6299787005 * lb,
  );
  return [
    0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  ] as const;
}
function fromOklab(L: number, a: number, b: number) {
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  return [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
  ];
}
/** OKLCH of a hex colour. */
export function oklch(color: Hex): Oklch {
  const [l, a, b] = toOklab(rgb(color));
  const h = (Math.atan2(b, a) * 180) / Math.PI;
  return { l, c: Math.hypot(a, b), h: h < 0 ? h + 360 : h };
}
/** The hex colour of an OKLCH value; chroma is reduced until it fits sRGB. */
export function fromOklch({ l, c, h }: Oklch): Hex {
  const lightness = Math.min(1, Math.max(0, l));
  const rad = (h * Math.PI) / 180;
  const linear = (chroma: number) =>
    fromOklab(lightness, chroma * Math.cos(rad), chroma * Math.sin(rad));
  const inside = (v: number[]) => v.every((x) => x >= -1e-6 && x <= 1 + 1e-6);
  let [low, high] = [0, Math.max(0, c)];
  if (!inside(linear(high)))
    for (let i = 0; i < 24; i++) {
      const mid = (low + high) / 2;
      if (inside(linear(mid))) low = mid;
      else high = mid;
    }
  else low = high;
  const v = linear(low);
  return hex(
    v.map((x) => gamma(Math.min(1, Math.max(0, x))) * 255) as [
      number,
      number,
      number,
    ],
  );
}
const hueDistance = (a: number, b: number) => {
  const d = Math.abs(a - b) % 360;
  return d > 180 ? 360 - d : d;
};

// ---------------------------------------------------------- palette extraction

/** Decoded RGBA pixels of one brand image; the logo weighs more than mood images. */
export type PixelSample = {
  data: ArrayLike<number>;
  width: number;
  height: number;
  weight: number;
};
/** The brand colours found in the images. */
export type BrandPalette = {
  primary: Hex;
  secondary: Hex | null;
  neutral: Hex;
};
type Bucket = { r: number; g: number; b: number; w: number };

/**
 * Deterministic palette extraction: opaque pixels fall into 4-bit sRGB buckets,
 * chromatic buckets are grouped into 15° hue bins weighted by chroma, and the
 * strongest bin is the primary; a second bin at least 45° away with a tenth of
 * its weight is the secondary. Without chromatic pixels the darkest dominant
 * tone is the primary (a monochrome brand). The same pixels give the same palette.
 */
export function extractPalette(samples: PixelSample[]): BrandPalette {
  const buckets = new Map<number, Bucket>();
  for (const { data, weight } of samples)
    for (let i = 0; i + 3 < data.length; i += 4) {
      if (data[i + 3] < 128) continue;
      const [r, g, b] = [data[i], data[i + 1], data[i + 2]];
      const key = ((r >> 4) << 8) | ((g >> 4) << 4) | (b >> 4);
      const bucket = buckets.get(key) ?? { r: 0, g: 0, b: 0, w: 0 };
      bucket.r += r * weight;
      bucket.g += g * weight;
      bucket.b += b * weight;
      bucket.w += weight;
      buckets.set(key, bucket);
    }
  const colours = [...buckets.entries()]
    .sort(([a], [b]) => a - b)
    .map(([, v]) => {
      const color = hex([v.r / v.w, v.g / v.w, v.b / v.w]);
      return { color, w: v.w, ...oklch(color) };
    });
  const bins = new Map<
    number,
    { score: number; w: number; L: number; a: number; b: number }
  >();
  for (const c of colours) {
    if (c.c < 0.05 || c.l < 0.25 || c.l > 0.92) continue;
    const bin = Math.floor(c.h / 15) % 24;
    const entry = bins.get(bin) ?? { score: 0, w: 0, L: 0, a: 0, b: 0 };
    const [L, a, b] = toOklab(rgb(c.color));
    entry.score += c.w * c.c;
    entry.w += c.w;
    entry.L += L * c.w;
    entry.a += a * c.w;
    entry.b += b * c.w;
    bins.set(bin, entry);
  }
  const ranked = [...bins.entries()]
    .map(([bin, e]) => {
      const color = hex(
        fromOklab(e.L / e.w, e.a / e.w, e.b / e.w).map(
          (x) => gamma(Math.min(1, Math.max(0, x))) * 255,
        ) as [number, number, number],
      );
      return { bin, score: e.score, color, h: oklch(color).h };
    })
    .sort((x, y) => y.score - x.score || x.bin - y.bin);
  if (!ranked.length) {
    const total = colours.reduce((sum, c) => sum + c.w, 0);
    const dark = colours
      .filter((c) => c.w >= total * 0.05 && c.l < 0.6)
      .sort((x, y) => x.l - y.l || x.color.localeCompare(y.color));
    const primary = dark[0]?.color ?? "#2b2b2b";
    return { primary, secondary: null, neutral: neutralOf(primary) };
  }
  const [first] = ranked;
  const second = ranked.find(
    (r) => hueDistance(r.h, first.h) >= 45 && r.score >= first.score * 0.1,
  );
  return {
    primary: first.color,
    secondary: second?.color ?? null,
    neutral: neutralOf(first.color),
  };
}
const neutralOf = (color: Hex) => {
  const { h, c } = oklch(color);
  return fromOklch({ l: 0.55, c: Math.min(0.02, c * 0.2), h });
};

/** Corner radius suggested by a logo's silhouette: empty bounding-box corners read as round. */
export type RadiusScale = "sharp" | "soft" | "round";
export function logoShape(sample: PixelSample): RadiusScale {
  const { data, width, height } = sample;
  if (!width || !height) return "soft";
  const at = (x: number, y: number) => (y * width + x) * 4;
  // Opaque corner pixels are the background colour; a transparent logo has none.
  const background = [0, width - 1, (height - 1) * width, width * height - 1]
    .filter((p) => data[p * 4 + 3] >= 128)
    .map((p) => toOklab([data[p * 4], data[p * 4 + 1], data[p * 4 + 2]]));
  const ink = (x: number, y: number) => {
    const i = at(x, y);
    if (data[i + 3] < 128) return false;
    const [L, a, b] = toOklab([data[i], data[i + 1], data[i + 2]]);
    return background.every(
      ([L2, a2, b2]) => Math.hypot(L - L2, a - a2, b - b2) > 0.08,
    );
  };
  let [x0, y0, x1, y1] = [width, height, -1, -1];
  for (let y = 0; y < height; y++)
    for (let x = 0; x < width; x++)
      if (ink(x, y)) {
        x0 = Math.min(x0, x);
        y0 = Math.min(y0, y);
        x1 = Math.max(x1, x);
        y1 = Math.max(y1, y);
      }
  if (x1 - x0 < 4 || y1 - y0 < 4) return "soft";
  const cw = Math.max(1, Math.round((x1 - x0 + 1) * 0.15));
  const ch = Math.max(1, Math.round((y1 - y0 + 1) * 0.15));
  const coverage = (sx: number, sy: number) => {
    let hit = 0;
    for (let y = sy; y < sy + ch; y++)
      for (let x = sx; x < sx + cw; x++) hit += ink(x, y) ? 1 : 0;
    return hit / (cw * ch);
  };
  const corners =
    (coverage(x0, y0) +
      coverage(x1 - cw + 1, y0) +
      coverage(x0, y1 - ch + 1) +
      coverage(x1 - cw + 1, y1 - ch + 1)) /
    4;
  return corners < 0.35 ? "round" : corners < 0.7 ? "soft" : "sharp";
}

// ------------------------------------------------------------ vetted choices

/** Vetted font stacks. They name locally installed families only: no web font is fetched, so no shopper data leaves the shop. */
export const fontStacks = {
  "neo-grotesque":
    'Inter, Roboto, "Helvetica Neue", "Arial Nova", "Nimbus Sans", Arial, sans-serif',
  humanist:
    'Seravek, "Gill Sans Nova", Ubuntu, Calibri, "DejaVu Sans", source-sans-pro, sans-serif',
  geometric:
    'Avenir, Montserrat, Corbel, "URW Gothic", source-sans-pro, sans-serif',
  transitional:
    'Charter, "Bitstream Charter", "Sitka Text", Cambria, Georgia, serif',
  "old-style":
    '"Iowan Old Style", "Palatino Linotype", "URW Palladio L", P052, serif',
  didone:
    'Didot, "Bodoni MT", "Noto Serif Display", "URW Palladio L", P052, Sylfaen, serif',
  slab: 'Rockwell, "Rockwell Nova", "Roboto Slab", "DejaVu Serif", "Sitka Small", serif',
  mono: 'ui-monospace, "Cascadia Code", "Source Code Pro", Menlo, Consolas, "DejaVu Sans Mono", monospace',
} as const;
export type FontId = keyof typeof fontStacks;
/** Vetted heading/body pairings; a theme always names one of these. */
export const fontPairings = {
  classic: { heading: "transitional", body: "neo-grotesque" },
  editorial: { heading: "didone", body: "neo-grotesque" },
  modern: { heading: "geometric", body: "neo-grotesque" },
  friendly: { heading: "humanist", body: "humanist" },
  crafted: { heading: "slab", body: "humanist" },
  timeless: { heading: "old-style", body: "old-style" },
  technical: { heading: "mono", body: "neo-grotesque" },
} as const satisfies Record<string, { heading: FontId; body: FontId }>;
export type PairingId = keyof typeof fontPairings;
export const radiusScales = {
  sharp: { sm: 2, md: 4, lg: 6, site: 4 },
  soft: { sm: 8, md: 12, lg: 16, site: 14 },
  round: { sm: 12, md: 18, lg: 28, site: 24 },
} as const;
export type Density = "compact" | "regular" | "airy";
export const densityScales = {
  compact: { factor: 0.85, site: 28 },
  regular: { factor: 1, site: 40 },
  airy: { factor: 1.25, site: 60 },
} as const;
export const motionIntensities = ["off", "subtle", "expressive"] as const;
export type MotionIntensity = (typeof motionIntensities)[number];

// ---------------------------------------------------------- the theme document

/** The colour roles a brand theme sets; they replace the light `--vy-color-*` tokens on the shop. */
export const brandColors = [
  "bg",
  "surface",
  "panel",
  "hover",
  "border",
  "muted",
  "text",
  "accent",
  "accentText",
  "accentSoft",
  "accentStrong",
  "accentBorder",
  "focus",
] as const;
export type BrandColor = (typeof brandColors)[number];
/**
 * The contrast pairs every brand theme meets: 4.5:1 for text (WCAG 1.4.3 AA),
 * 3:1 for the focus ring (1.4.11). Rust validates the same list on every save.
 */
export const contrastPairs: [BrandColor, BrandColor, number][] = [
  ["text", "bg", 4.5],
  ["text", "surface", 4.5],
  ["text", "panel", 4.5],
  ["text", "hover", 4.5],
  ["text", "accentSoft", 4.5],
  ["muted", "bg", 4.5],
  ["muted", "surface", 4.5],
  ["muted", "panel", 4.5],
  ["accent", "bg", 4.5],
  ["accent", "surface", 4.5],
  ["accent", "panel", 4.5],
  ["accent", "accentSoft", 4.5],
  ["accentStrong", "surface", 4.5],
  ["accentText", "accent", 4.5],
  ["accentText", "accentStrong", 4.5],
  ["focus", "bg", 3],
  ["focus", "surface", 3],
];
const hexField = z.string().regex(hexPattern);
const fontField = z.enum(Object.keys(fontStacks) as [FontId, ...FontId[]]);
/** The stored brand token document (`theme.brand` on a shop home page). */
export const brandThemeSchema = z
  .object({
    version: z.literal(1),
    source: z.array(z.string().min(1).max(200)).max(6),
    palette: z
      .object({
        primary: hexField,
        secondary: hexField.nullable(),
        neutral: hexField,
      })
      .strict(),
    colors: z
      .object(
        Object.fromEntries(brandColors.map((k) => [k, hexField])) as Record<
          BrandColor,
          typeof hexField
        >,
      )
      .strict(),
    fonts: z
      .object({
        pairing: z.enum(
          Object.keys(fontPairings) as [PairingId, ...PairingId[]],
        ),
        heading: fontField,
        body: fontField,
      })
      .strict(),
    radius: z.enum(["sharp", "soft", "round"]),
    density: z.enum(["compact", "regular", "airy"]),
  })
  .strict();
export type BrandTheme = z.infer<typeof brandThemeSchema>;

/** Every contrast pair below its minimum, with the measured ratio. */
export function contrastFindings(colors: Record<BrandColor, Hex>) {
  return contrastPairs
    .map(([fg, bg, min]) => ({
      fg,
      bg,
      min,
      ratio: Math.round(contrast(colors[fg], colors[bg]) * 100) / 100,
    }))
    .filter((p) => p.ratio < p.min);
}
/** Untrusted theme input (a model, an import, a request) as a valid theme, or the reason it is refused. */
export function parseBrandTheme(
  value: unknown,
): { theme: BrandTheme } | { error: string } {
  const parsed = brandThemeSchema.safeParse(value);
  if (!parsed.success) return { error: "shape" };
  const { pairing, heading, body } = parsed.data.fonts;
  if (
    fontPairings[pairing].heading !== heading ||
    fontPairings[pairing].body !== body
  )
    return { error: "fonts" };
  if (contrastFindings(parsed.data.colors).length) return { error: "contrast" };
  return { theme: parsed.data };
}

/** Darkens (or lightens) a foreground in OKLCH steps until it meets `min` against every background. */
function meet(fg: Oklch, backgrounds: Hex[], min: number, darker = true) {
  let next = { ...fg };
  for (let i = 0; i < 100; i++) {
    const color = fromOklch(next);
    if (backgrounds.every((b) => contrast(color, b) >= min)) return color;
    next = { ...next, l: next.l + (darker ? -0.01 : 0.01) };
  }
  return darker ? "#000000" : "#ffffff";
}

/** What the merchant chose instead of the derived defaults. */
export type BrandChoices = {
  pairing?: PairingId;
  radius?: RadiusScale;
  density?: Density;
};
/** Defaults the colours suggest: vivid brands get a livelier pairing and motion, monochrome brands an editorial one. */
export function suggestions(palette: BrandPalette): {
  pairing: PairingId;
  motion: MotionIntensity;
} {
  const { c, h } = oklch(palette.primary);
  if (c < 0.05) return { pairing: "editorial", motion: "subtle" };
  const warm = h < 110 || h >= 330;
  if (c >= 0.15)
    return { pairing: warm ? "friendly" : "modern", motion: "expressive" };
  return { pairing: "classic", motion: "subtle" };
}

/** A complete light theme for the palette: every contrast pair meets WCAG AA. */
export function deriveBrandTheme(
  palette: BrandPalette,
  choices: BrandChoices & { source?: string[]; shape?: RadiusScale } = {},
): BrandTheme {
  const brand = oklch(palette.primary);
  const h = brand.h;
  const tint = Math.min(0.02, brand.c * 0.15);
  const surface = fromOklch({ l: 0.995, c: tint * 0.25, h });
  const bg = fromOklch({ l: 0.975, c: tint * 0.6, h });
  const panel = fromOklch({ l: 0.955, c: tint, h });
  const hover = fromOklch({ l: 0.94, c: Math.min(0.035, brand.c * 0.3), h });
  const border = fromOklch({ l: 0.885, c: tint, h });
  const accentSoft = fromOklch({
    l: 0.95,
    c: Math.min(0.04, brand.c * 0.3),
    h,
  });
  const light = [bg, surface, panel, hover, accentSoft];
  const text = meet({ l: 0.24, c: tint * 1.2, h }, light, 7);
  const muted = meet({ l: 0.5, c: tint * 1.5, h }, [bg, surface, panel], 4.5);
  const accentSeed = { l: Math.min(brand.l, 0.62), c: brand.c, h };
  const accent = meet(accentSeed, [bg, surface, panel, accentSoft], 4.5);
  const strong = meet(
    { ...oklch(accent), l: oklch(accent).l - 0.06 },
    [surface],
    4.5,
  );
  const white = surface;
  const accentText =
    contrast(white, accent) >= 4.5 && contrast(white, strong) >= 4.5
      ? white
      : text;
  const accentBorder = fromOklch({
    l: 0.8,
    c: Math.min(0.09, brand.c * 0.6),
    h,
  });
  const fallback = suggestions(palette);
  const pairing = choices.pairing ?? fallback.pairing;
  return {
    version: 1,
    source: (choices.source ?? []).slice(0, 6),
    palette,
    colors: {
      bg,
      surface,
      panel,
      hover,
      border,
      muted,
      text,
      accent,
      accentText,
      accentSoft,
      accentStrong: strong,
      accentBorder,
      focus: accent,
    },
    fonts: { pairing, ...fontPairings[pairing] },
    radius: choices.radius ?? choices.shape ?? "soft",
    density: choices.density ?? "regular",
  };
}

const kebab = (key: string) =>
  key.replace(/[A-Z]/g, (c) => "-" + c.toLowerCase());
const baseSpace = { 1: 4, 2: 8, 3: 12, 4: 16, 5: 20, 6: 24, 8: 32, 10: 40 };
/**
 * The custom properties a brand theme sets on the shop root: the light colour,
 * font, radius and spacing tokens, plus the Veyndo palette aliases the
 * storefront styles read, re-declared so they resolve against the shop's values.
 */
export function brandVariables(theme: BrandTheme): Record<string, string> {
  const vars: Record<string, string> = {};
  for (const key of brandColors)
    vars[`--vy-color-${kebab(key)}`] = theme.colors[key];
  vars["--vy-font-family"] = fontStacks[theme.fonts.body];
  vars["--vy-font-heading"] = fontStacks[theme.fonts.heading];
  const radius = radiusScales[theme.radius];
  for (const key of ["sm", "md", "lg"] as const)
    vars[`--vy-radius-${key}`] = `${radius[key]}px`;
  const { factor } = densityScales[theme.density];
  for (const [step, px] of Object.entries(baseSpace))
    vars[`--vy-space-${step}`] = `${Math.round(px * factor)}px`;
  vars["--veyndo-blue-deep"] = "var(--vy-color-focus)";
  vars["--veyndo-blue-text"] = "var(--vy-color-accent)";
  vars["--veyndo-blue-ink"] = "var(--vy-color-text)";
  vars["--veyndo-blue-soft"] = "var(--vy-color-accent-soft)";
  vars["--veyndo-blue-hover"] = "var(--vy-color-hover)";
  return vars;
}
/** The legacy page-theme fields that follow a brand theme, so site sections match the shop. */
export function brandSiteFields(theme: BrandTheme) {
  const body = theme.fonts.body;
  return {
    accent: theme.colors.accent,
    background: theme.colors.surface,
    foreground: theme.colors.text,
    font: (body === "mono"
      ? "mono"
      : ["transitional", "old-style", "didone", "slab"].includes(body)
        ? "serif"
        : "sans") as "sans" | "serif" | "mono",
    radius: radiusScales[theme.radius].site,
    spacing: densityScales[theme.density].site,
  };
}

/**
 * Theme variables received from another frame (MCP App host context
 * `styles.variables`): only `--vy-*` names with plain values (colours, lengths,
 * font stacks) pass; `url(`, `var(`, `expression` and declaration breaks are dropped.
 */
export function safeThemeVariables(value: unknown): Record<string, string> {
  if (!value || typeof value !== "object" || Array.isArray(value)) return {};
  return Object.fromEntries(
    Object.entries(value as Record<string, unknown>)
      .filter(
        (entry): entry is [string, string] =>
          /^--vy-[a-z0-9-]{1,40}$/.test(entry[0]) &&
          typeof entry[1] === "string" &&
          entry[1].length <= 300 &&
          /^[\w\s#.,"'%-]*$/.test(entry[1]),
      )
      .slice(0, 64),
  );
}
