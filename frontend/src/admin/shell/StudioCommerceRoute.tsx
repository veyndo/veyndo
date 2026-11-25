/** Installed app surfaces (including the marketplace workspace) stay within the active Studio workspace. */
import { lazy } from "react";
import { AppSurfaceView, useAppRegistry } from "../../shared/apps/AppSurfaces";
import { useCommerceNavText } from "./commerce-nav-i18n";
import { useStudio } from "./StudioContext";

const MarketplacePanel = lazy(
  () =>
    import("../../../../extensions/apps/marketplaces/studio/MarketplacePanel"),
);

export default function StudioCommerceRoute() {
  const { appSurface, b2bFeatures, request, access } = useStudio();
  const registry = useAppRegistry(),
    t = useCommerceNavText();
  if (appSurface) {
    if (appSurface.app === "edi" && (!b2bFeatures?.enabled || !b2bFeatures.edi))
      return null;
    if (appSurface.app !== "marketplaces")
      return <AppSurfaceView selected={appSurface} />;
    const market = registry.surfaces.find(
      (s) => s.app === "marketplaces" && s.surface.id === appSurface.surface.id,
    );
    if (!market || !access.includes("apps.manage")) return null;
    return (
      <div className="studio-page marketplace-workspace">
        <header className="page-intro">
          <h1>{t("marketplaces")}</h1>
          <p>{t("marketplaceHint")}</p>
        </header>
        <MarketplacePanel
          request={request}
          manage
          mainLocale={market.mainLocale ?? registry.mainLocale}
        />
      </div>
    );
  }
  return null;
}
