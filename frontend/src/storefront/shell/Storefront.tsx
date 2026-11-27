/** Storefront composition root: cart context, routes, customer account and checkout. */
import { forApps } from "../../shared/api/types";
import {
  checkoutParent,
  returnFromCheckout,
} from "../../../../components/checkout/ui/storefront/checkout/embedded-checkout";
import { CurrencyContext } from "../../shared/i18n/i18n";
import { shopApi, type Cart } from "../../shared/api/shop-api";
import {
  AppSurfaceProvider,
  AppSurfaceSlot,
  StorefrontAppPage,
} from "../../shared/apps/AppSurfaces";
import "../../shared/styles/apps.css";
import "../../shared/styles/workbench.css";
import Icon from "../../shared/ui/Icon";
import CustomerAccount from "../../../../components/customers/ui/storefront/account/CustomerAccount";
import PrivacyProvider from "../../../../components/shop/ui/storefront/legal/PrivacyProvider";
import LegalDocument from "../../../../components/shop/ui/storefront/legal/LegalDocument";
import ConsumerRequestForm from "../../../../components/shop/ui/storefront/legal/ConsumerRequestForm";
import BehaviorAnalytics from "./BehaviorAnalytics";
import ShopAnalytics from "../../../../extensions/apps/google-analytics/storefront/ShopAnalytics";
import ProductPage from "../../../../components/catalog/ui/storefront/catalog/ProductPage";
import { returnToCollection } from "../../../../components/catalog/ui/storefront/catalog/product-url";
import OrderCompletion from "../../../../components/checkout/ui/storefront/checkout/OrderCompletion";
import CheckoutPanel from "../../../../components/checkout/ui/storefront/checkout/CheckoutPanel";
import PaymentSession from "../../../../components/checkout/ui/storefront/checkout/PaymentSession";
import { paymentReturnAttempt } from "../../../../components/checkout/ui/storefront/checkout/payment-return";
import "../styles/shop.css";

import { StorefrontContext } from "./StorefrontContext";
import CompanyLegalPage from "./CompanyLegalPage";
import StorefrontFooter from "./StorefrontFooter";
import "../styles/company-identity.css";
import StorefrontHeader from "./StorefrontHeader";
import StorefrontHome from "./StorefrontHome";
import PublishedSite from "../../../../extensions/apps/site-builder/storefront/PublishedSite";
import ShopSite from "../../../../extensions/apps/site-builder/storefront/ShopSite";
import { useStorefrontController } from "./useStorefrontController";
import "../styles/storefront-polish.css";
import "../styles/experience-polish.css";
import ChannelPreview, { channelPreview } from "./ChannelPreview";
import { ShopBrandProvider } from "../../shared/content/shop-brand";
export default function Storefront(props: { onMerchant: () => void }) {
  const c = useStorefrontController(props);
  const returnedAttempt = paymentReturnAttempt(location.hash);
  const {
    cart,
    shopTenant,
    salesChannel,
    products,
    bag,
    order,
    w,
    error,
    s,
    setError,
    id,
    appPath,
    busy,
    add,
    save,
    account,
    setAccount,
    setBag,
    quantity,
    selection,
    buy,
  } = c;
  return (
    <CurrencyContext.Provider value={cart?.price.currency ?? "EUR"}>
      <StorefrontContext.Provider value={c}>
        <ShopBrandProvider value={c.brand}>
          <PrivacyProvider token={cart?.token}>
            <AppSurfaceProvider
              public
              request={forApps((path, body) =>
                shopApi(path, body, cart?.token),
              )}
              scopeKey={`${shopTenant}:${salesChannel}`}
            >
              <PublishedSite>
                <div className="shop">
                  <ChannelPreview />
                  <BehaviorAnalytics
                    shop={shopTenant}
                    channel={salesChannel}
                    cart={cart}
                    id={id}
                    bag={bag}
                    query={c.query}
                    route={appPath}
                  />
                  {!channelPreview() && (
                    <ShopAnalytics
                      shop={shopTenant}
                      channel={salesChannel}
                      products={products}
                      cart={cart}
                      bag={bag}
                      order={order}
                    />
                  )}
                  {new URLSearchParams(location.search).get("sandbox") ===
                    "1" && (
                    <div className="sandbox-banner">
                      {w("stage")} · {w("exclusion")}
                    </div>
                  )}
                  {!checkoutParent() && <StorefrontHeader />}
                  <AppSurfaceSlot
                    location="storefront.header"
                    context={{ salesChannel }}
                  />
                  {error && (
                    <div role="alert" className="shop-error">
                      {error}
                      <button
                        aria-label={s("close")}
                        onClick={() => setError("")}
                      >
                        <Icon name="close" />
                      </button>
                    </div>
                  )}
                  {appPath === "#order-confirmed" && order ? (
                    <OrderCompletion
                      order={order}
                      onBack={returnToCollection}
                    />
                  ) : returnedAttempt &&
                    localStorage.getItem(
                      `rac-payment-token:${returnedAttempt}`,
                    ) ? (
                    <main className="shop-content">
                      <PaymentSession
                        id={returnedAttempt}
                        token={localStorage.getItem(
                          `rac-payment-token:${returnedAttempt}`,
                        )!}
                      />
                    </main>
                  ) : appPath.startsWith("#app/") ? (
                    <StorefrontAppPage path={appPath} />
                  ) : appPath === "#withdrawal" ||
                    appPath === "#privacy-rights" ? (
                    <ConsumerRequestForm
                      token={cart?.token}
                      withdrawal={appPath === "#withdrawal"}
                    />
                  ) : appPath.startsWith("#legal/") ? (
                    <LegalDocument kind={appPath.slice(7)} />
                  ) : appPath === "#legal" ? (
                    <CompanyLegalPage />
                  ) : checkoutParent() ? null : id ? (
                    <ProductPage
                      id={id}
                      cart={cart}
                      busy={busy}
                      onAdd={add}
                      onConfigure={c.actions.configureProduct}
                    />
                  ) : (
                    <ShopSite fallback={<StorefrontHome />} />
                  )}
                  {!checkoutParent() && <StorefrontFooter name={c.brand} />}
                  {account && !channelPreview() && (
                    <CustomerAccount
                      initialPage={account}
                      cart={cart}
                      onCart={save}
                      onClose={() => setAccount(false)}
                    />
                  )}
                  {bag && (
                    <CheckoutPanel
                      cart={cart}
                      requestError={error}
                      order={order}
                      busy={busy}
                      onClose={() => {
                        returnFromCheckout();
                        setBag(false);
                      }}
                      onQuantity={quantity}
                      onSelection={selection}
                      onCart={save}
                      preview={channelPreview()}
                      onBuy={buy}
                      onCoupons={async (codes) => {
                        const result = await shopApi<Cart>(
                          "/store-api/checkout/coupons",
                          { codes, revision: cart?.revision },
                          cart?.token,
                          "PUT",
                        );
                        save(result);
                      }}
                    />
                  )}
                </div>
              </PublishedSite>
            </AppSurfaceProvider>
          </PrivacyProvider>
        </ShopBrandProvider>
      </StorefrontContext.Provider>
    </CurrencyContext.Provider>
  );
}
