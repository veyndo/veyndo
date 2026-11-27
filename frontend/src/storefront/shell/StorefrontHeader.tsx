/** StorefrontHeader: storefront view composed from the scoped cart/controller. */
import { commercePage } from "../../../../extensions/apps/site-builder/commerce-layout";
import { usePublishedSite } from "../../../../extensions/apps/site-builder/storefront/PublishedSite";
import { openConsent } from "../../shared/legal/consent-store";
import { StorefrontAppNavigation } from "../../shared/apps/AppSurfaces";
import StorefrontHeaderView from "./StorefrontHeaderView";
import StorefrontCurrency from "./StorefrontCurrency";
import "../../shared/styles/currencies.css";
import StorefrontLanguage from "./StorefrontLanguage";
import "../../shared/styles/apps.css";
import "../../shared/styles/workbench.css";
import "../styles/shop.css";

import { useStorefront } from "./StorefrontContext";
export default function StorefrontHeader() {
  const {
    company,
    salesChannel,
    busy,
    cart,
    setAccount,
    onMerchant,
    adaptation,
    brand,
    openBag,
  } = useStorefront();
  const { pages } = usePublishedSite();
  return (
    <StorefrontHeaderView
      displayName={brand}
      company={company}
      busy={busy}
      business={cart?.customerGroup === "business"}
      businessDisabled={!cart}
      adaptation={adaptation}
      count={
        cart?.status === "completed"
          ? 0
          : (cart?.lineItems.reduce((n, i) => n + i.quantity, 0) ?? 0)
      }
      collectionLayout={!!commercePage(pages, salesChannel, "category")}
      navigation={<StorefrontAppNavigation />}
      onAccount={() => setAccount("overview")}
      onMerchant={onMerchant}
      onConsent={openConsent}
      onBag={openBag}
      onBusiness={() => setAccount("business")}
    >
      <StorefrontLanguage />
      <StorefrontCurrency />
    </StorefrontHeaderView>
  );
}
