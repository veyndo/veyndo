/** Storefront-kit parts placed on site pages: placeable contracts, product resolution and the server schema catalogue. */
import {
  productCard,
  productSummary,
  type AnyPart,
} from "../../../frontend/src/kit/storefront";
import { productURL } from "../../../components/catalog/ui/storefront/catalog/product-url";
import { mediaImage } from "../../../frontend/src/shared/media/product-media";
import type { Block, PartRef } from "./model";
import type { SiteProduct } from "./Renderer";

/** Parts a merchant can place on a page; each resolves its props from one shop product. */
export const siteParts: readonly AnyPart[] = [productCard, productSummary];
export const sitePart = (id: string) => siteParts.find((p) => p.id === id);

/** A part block with the part's default configuration. */
export function newPartRef(part: AnyPart): PartRef {
  const parsed = part.parseConfig({});
  const defaults = parsed.ok ? (parsed.config as Record<string, unknown>) : {};
  return { id: part.id, version: part.version, config: { ...defaults } };
}

type Schema = { properties?: Record<string, { format?: string }> };
/** Configuration fields holding product IDs (`format: product-id`). */
export function productFields(part: AnyPart): string[] {
  const schema = part.configSchema() as Schema;
  return Object.entries(schema.properties ?? {})
    .filter(([, spec]) => spec.format === "product-id")
    .map(([field]) => field);
}
/** Product IDs a part block refers to, for bounded catalogue resolution. */
export function partProducts(block: Block): string[] {
  const part = block.part && sitePart(block.part.id);
  if (!part || !block.part) return [];
  return productFields(part)
    .map((field) => block.part!.config[field])
    .filter((id): id is string => typeof id === "string" && !!id);
}

/** What the server validates: version and configuration JSON Schema per placeable part. */
export function serverSchemas() {
  return Object.fromEntries(
    siteParts.map((part) => [
      part.id,
      { version: part.version, config: part.configSchema() },
    ]),
  );
}

/** Resolves a placed part with live shop data; renders nothing until the data is valid. */
export function SitePart({
  block,
  products,
  language,
  onAdd,
}: {
  block: Block;
  products: SiteProduct[];
  language: string;
  onAdd?: (id: string, quantity?: number) => void;
}) {
  const part = block.part && sitePart(block.part.id);
  if (!part || !block.part) return null;
  const config = part.parseConfig(block.part.config);
  if (!config.ok) return null;
  const settings = config.config as { productId?: string; priceKind?: string };
  const p = products.find((product) => product.id === settings.productId);
  if (!p) return null;
  const props = part.parse({
    locale: language,
    priceKind: settings.priceKind,
    product: {
      id: p.id,
      name: p.name,
      url: productURL(p),
      image: mediaImage(p.media) || undefined,
      price: p.price,
      currency: p.currency ?? "EUR",
      stock: p.stock ?? 0,
      description: p.description || undefined,
    },
  });
  if (!props.ok) return null;
  const Component = part.Component;
  return (
    <Component
      {...(props.props as object)}
      onAddToCart={
        onAdd
          ? (detail: { productId: string; quantity: number }) =>
              onAdd(detail.productId, detail.quantity)
          : undefined
      }
    />
  );
}

/** Vocabulary keys of placeable parts and their configuration fields. */
export const partWords = {
  "product-card": ["partProductCard", "partProductCardHint"],
  "product-summary": ["partProductSummary", "partProductSummaryHint"],
} as const satisfies Record<string, readonly [string, string]>;
export const partFieldWords = {
  productId: "partProduct",
  priceKind: "partPriceKind",
} as const;
export const partChoiceWords = { gross: "partGross", net: "partNet" } as const;
