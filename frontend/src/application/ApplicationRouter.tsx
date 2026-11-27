/** Select independent lazy applications; send sign-in surfaces to the Studio origin; contain failed imports and reset boundaries on navigation. */
import { lazy, Suspense, useEffect, useState } from "react";
import { useLocale } from "../shared/i18n/i18n";
import WorkspaceBoundary from "../shared/ui/WorkspaceBoundary";
import "../kit/tokens";
import {
  shopScope,
  storefrontURL,
  canonicalShopURL,
  hostnameShop,
} from "../shared/api/shop-scope";
import { studioAddress, useStudioOrigin } from "../shared/api/studio-origin";
const AdminHub = lazy(() => import("../platform/AdminHub"));
const Merchant = lazy(() => import("../admin/shell/Merchant"));
const Storefront = lazy(() => import("../storefront/shell/Storefront"));
const PlatformConsole = lazy(() => import("../platform/PlatformConsole"));
export default function ApplicationRouter() {
  const { t } = useLocale();
  const canonical = canonicalShopURL(new URL(location.href));
  useEffect(() => {
    if (canonical) location.replace(canonical);
  }, [canonical]);
  const [hub, setHub] = useState(
    location.hostname === "admin.veyndo.ai" && !location.hash,
  );
  const [platform, setPlatform] = useState(
    location.hash === "#platform" ||
      (!location.hash &&
        !location.search &&
        (location.hostname === "veyndo.ai" ||
          location.hostname === "app.veyndo.ai" ||
          location.hostname.endsWith(".code.run"))),
  );
  const [admin, setAdmin] = useState(
    ["#merchant", "#studio-content", "#login"].includes(location.hash),
  );
  useEffect(() => {
    const change = () => {
      setHub(location.hostname === "admin.veyndo.ai" && !location.hash);
      setPlatform(
        location.hash === "#platform" ||
          (!location.hash &&
            !location.search &&
            (location.hostname === "veyndo.ai" ||
              location.hostname === "app.veyndo.ai" ||
              location.hostname.endsWith(".code.run"))),
      );
      setAdmin(
        ["#merchant", "#studio-content", "#login"].includes(location.hash),
      );
      window.scrollTo({ top: 0, behavior: "instant" });
    };
    window.addEventListener("hashchange", change);
    return () => window.removeEventListener("hashchange", change);
  }, []);
  // Sign-in works only on the configured Studio origin. On another host (the service
  // directory) the operator console and Studio would fail as "no access": go there instead.
  const studio = useStudioOrigin(!canonical && (platform || admin));
  const moving = !!studio && (platform || admin);
  useEffect(() => {
    if (moving && studio)
      location.replace(studioAddress(studio, hostnameShop()));
  }, [moving, studio]);
  if (canonical || moving) return <div role="status">…</div>;
  const content = hub ? (
    <AdminHub />
  ) : platform ? (
    <PlatformConsole />
  ) : admin ? (
    <Merchant
      onExit={() => {
        location.assign(storefrontURL(shopScope()));
      }}
      onChanged={async () => {}}
    />
  ) : (
    <Storefront
      onMerchant={() => {
        location.assign(storefrontURL(shopScope(), true));
      }}
    />
  );
  return (
    <WorkspaceBoundary
      key={platform ? "platform" : admin ? "studio" : "store"}
      title={t("failure")}
      retryLabel={t("refresh")}
      onRetry={() => window.location.reload()}
    >
      <Suspense fallback={<div role="status">…</div>}>{content}</Suspense>
    </WorkspaceBoundary>
  );
}
