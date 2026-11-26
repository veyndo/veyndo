/** Storefront adapter of the kit product card: maps a shop product to the part contract, keeps native URLs and texts. */
import { mediaImage } from "../../shared/media/product-media";
import { productURL } from "../../../../components/catalog/ui/storefront/catalog/product-url";
import { useShopText } from "../../shared/i18n/shop-i18n";
import type { Product } from "../../shared/api/shop-api";
import { productCard } from "../../kit/storefront";
export type TileProduct = Pick<Product, "id" | "name" | "price" | "media"> &
  Partial<
    Pick<
      Product,
      "description" | "stock" | "category" | "calculated_price" | "extra"
    >
  >;
const Card = productCard.Component;
export default function ProductTile({
  product: p,
  currency,
  locale,
  business = false,
  onViewed,
}: {
  product: TileProduct;
  /** Display currency of the price; defaults to the shop's current currency. */
  currency?: string;
  /** Display language; defaults to the shop's current language. */
  locale?: string;
  business?: boolean;
  onViewed?: () => void;
}) {
  const shop = useShopText();
  const category = p.category ? shop.s(p.category) : undefined;
  return (
    <div className="shop-tile" onMouseEnter={onViewed}>
      <Card
        locale={locale ?? shop.locale}
        priceKind={business ? "net" : "gross"}
        product={{
          id: p.id,
          name: p.name,
          url: productURL(p),
          image: mediaImage(p.media) || undefined,
          description: p.description || undefined,
          category: category || undefined,
          price: p.calculated_price?.unitPrice ?? p.price,
          currency: currency ?? shop.currency,
          stock: p.stock ?? 0,
        }}
      />
    </div>
  );
}
