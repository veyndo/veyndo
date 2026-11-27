/** Product details render identically in Studio and the native storefront. */
import type { Product } from "../../../../../frontend/src/shared/api/shop-api";
import { useShopText } from "../../../../../frontend/src/shared/i18n/shop-i18n";
import { productSpecifications } from "../../../../../frontend/src/shared/content/product-specifications";
import { useShopBrand } from "../../../../../frontend/src/shared/content/shop-brand";
import RichDescription from "../../../../../frontend/src/shared/content/RichDescription";
import ProductAttachments from "./ProductAttachments";
export default function ProductDetails({
  product: p,
  language,
  preview = false,
}: {
  product: Product;
  language: string;
  preview?: boolean;
}) {
  const { s } = useShopText();
  const brand = useShopBrand();
  const contentLanguage = language.split("-")[0];
  const specs = productSpecifications(p.extra?.specifications, language);
  return (
    <div className="pdp-details">
      <section>
        <p className="shop-kicker">01 / {s("description")}</p>
        <h2>{s("description")}</h2>
        {Array.isArray(p.extra?.richDescription?.[contentLanguage]) ? (
          <RichDescription blocks={p.extra.richDescription[contentLanguage]} />
        ) : (
          <p>{p.description}</p>
        )}
        {!preview && <ProductAttachments id={p.id} />}
        <p className="product-material">
          {[s(p.properties.material ?? ""), brand && `${brand} /`]
            .filter(Boolean)
            .join(" · ")}
        </p>
      </section>
      <section>
        <p className="shop-kicker">02 / {s("properties")}</p>
        <h2>{s("properties")}</h2>
        <dl>
          {Object.entries({ ...p.properties, ...specs }).map(([key, value]) => (
            <div key={key}>
              <dt>{s(key)}</dt>
              <dd>{s(value)}</dd>
            </div>
          ))}
        </dl>
      </section>
    </div>
  );
}
