/** Product page purchase area: adapts the product detail to the kit product-summary part and keeps variant navigation, app slots and cart rules. */
import { productURL } from "./product-url";
import { BRAND } from "../../shared/ui/Brand";
import { useShopText } from "../../shared/i18n/shop-i18n";

import { shopApi, type Detail } from "../../shared/api/shop-api";
import AppSlot from "../../shared/apps/AppSlot";
import { AppSurfaceSlot } from "../../shared/apps/AppSurfaces";
import { productSummary } from "../../kit/storefront";
const Summary = productSummary.Component;
export type ProductPurchaseProps = {
  preview?: boolean;
  /** Display currency; defaults to the shop's current currency. */
  currency?: string;
  s: ReturnType<typeof useShopText>["s"];
  p: import("../../shared/api/shop-api").Product;
  data: import("../../shared/api/shop-api").Detail;
  tier: {
    quantity: number;
    price: import("../../shared/api/shop-api").Price;
    discountPercent: number;
  };
  groups: string[];
  variantsLoading: boolean;
  detailRequest: React.RefObject<number>;
  setVariantsLoading: React.Dispatch<React.SetStateAction<boolean>>;
  id: string;
  cart: import("../../shared/api/shop-api").Cart | undefined;
  setData: React.Dispatch<
    React.SetStateAction<import("../../shared/api/shop-api").Detail | undefined>
  >;
  setError: React.Dispatch<React.SetStateAction<string>>;
  onConfigure: import("../../shared/apps/AppSlot").Props["onConfigure"];
  quantity: number;
  setQuantity: React.Dispatch<React.SetStateAction<number>>;
  effective: number;
  busy: boolean;
  onAdd: (id: string, q: number) => void;
};
export default function ProductPurchase({
  preview = false,
  currency,
  s,
  p,
  data,
  tier,
  groups,
  variantsLoading,
  detailRequest,
  setVariantsLoading,
  id,
  cart,
  setData,
  setError,
  onConfigure,
  quantity,
  setQuantity,
  effective,
  busy,
  onAdd,
}: ProductPurchaseProps) {
  const { locale, currency: shopCurrency } = useShopText();
  const loadMore = async () => {
    if (preview || !data.variantsPagination.nextCursor) return;
    const request = detailRequest.current;
    setVariantsLoading(true);
    try {
      const page = await shopApi<Detail>(
        `/store-api/product/${encodeURIComponent(id)}?after=${encodeURIComponent(data.variantsPagination.nextCursor)}&limit=50`,
        {},
        cart?.token,
      );
      if (request === detailRequest.current)
        setData((previous) =>
          previous
            ? {
                ...previous,
                variants: [
                  ...new Map(
                    [...previous.variants, ...page.variants].map((v) => [
                      v.id,
                      v,
                    ]),
                  ).values(),
                ],
                variantsPagination: page.variantsPagination,
              }
            : previous,
        );
    } catch (e) {
      if (request === detailRequest.current) setError((e as Error).message);
    } finally {
      setVariantsLoading(false);
    }
  };
  const variantFor = (group: string, option: string) =>
    data.variants.find(
      (v) =>
        v.options[group] === option &&
        groups.every((g) => g === group || v.options[g] === p.options[g]),
    );
  const listPrice = tier.price.listPrice?.price;
  return (
    <div className="pdp-purchase">
      <Summary
        locale={locale}
        priceKind={data.taxStatus === "net" ? "net" : "gross"}
        shippingExtra
        product={{
          id: p.id,
          name: p.name,
          description: p.description || undefined,
          price: tier.price.unitPrice,
          listPrice:
            listPrice && listPrice > tier.price.unitPrice
              ? listPrice
              : undefined,
          currency: currency ?? shopCurrency,
          stock: p.stock,
          minPurchase: p.min_purchase,
          purchaseSteps: p.purchase_steps,
          maxPurchase: p.max_purchase ?? undefined,
          kicker: `${BRAND.name} / ${s(p.category)}`,
          sku: p.product_number || p.id,
          rating: {
            average: data.reviews.average,
            count: data.reviews.count,
          },
          reference: tier.price.referencePrice
            ? {
                price: tier.price.referencePrice.price,
                unit: `${tier.price.referencePrice.reference_unit} ${tier.price.referencePrice.unit_name}`,
              }
            : undefined,
        }}
        groups={groups.map((group) => ({
          id: group,
          label: s(group),
          selected: s(p.options[group]),
          options: [...new Set(data.variants.map((v) => v.options[group]))]
            .filter(Boolean)
            .map((option) => ({
              value: option,
              label: s(option),
              available: !!variantFor(group, option),
              selected: p.options[group] === option,
              swatch: group === "color" ? option : undefined,
            })),
        }))}
        moreVariants={
          !data.variantsPagination.nextCursor
            ? "none"
            : variantsLoading
              ? "loading"
              : "more"
        }
        showRules
        tiers={data.calculatedPrices.map((t) => ({
          quantity: t.quantity,
          unitPrice: t.price.unitPrice,
          discountPercent: t.discountPercent,
        }))}
        selectedTier={tier.quantity}
        delivery={
          data.delivery
            ? {
                minDays: data.delivery.minDays,
                maxDays: data.delivery.maxDays,
                method: s(data.delivery.method.name ?? "shipping"),
                country: s(data.country),
              }
            : undefined
        }
        quantity={quantity}
        disabled={preview}
        onQuantity={({ quantity: next, settled }) =>
          setQuantity(settled ? effective : next)
        }
        onSelect={({ group, value }) => {
          const match = variantFor(group, value);
          if (match && !preview) {
            history.pushState(null, "", productURL(match));
            window.dispatchEvent(new PopStateEvent("popstate"));
          }
        }}
        onOpen={({ id: target }) => {
          if (preview) return;
          if (target === "reviews")
            document
              .getElementById("product-reviews")
              ?.scrollIntoView({ behavior: "smooth" });
          if (target === "variants") void loadMore();
        }}
        onAddToCart={() => {
          if (!busy && cart && effective <= p.stock) onAdd(p.id, effective);
        }}
        slot={(name) =>
          name === "beforeBuy" && !preview ? (
            <>
              <AppSurfaceSlot
                location="product.detail"
                context={{ productId: p.id }}
              />
              <AppSlot
                productId={p.id}
                familyId={p.parent_id ?? p.id}
                cart={cart}
                onConfigure={onConfigure}
              />
            </>
          ) : null
        }
      />
    </div>
  );
}
