/** Renders one slot location: first-party contributions the current rights admit, then installed apps at the same location. */
import { Suspense, useContext } from "react";
import { AppSurfaceSlot } from "../../../shared/apps/AppSurfaces";
import WorkspaceBoundary from "../../../shared/ui/WorkspaceBoundary";
import { useLocale } from "../../../shared/i18n/i18n";
import { studioRegistry } from "./first-party";
import { StudioContext } from "../StudioContext";
import type { SlotContexts, SlotLocation } from "./types";

const appLocations: SlotLocation[] = [
  "admin.customer",
  "admin.order",
  "admin.order.general",
  "admin.product.general",
];

/** Outside a mounted Studio (for example in isolated views) only contributions without rights requirements render. */
export default function StudioSlot<L extends SlotLocation>({
  location,
  context,
}: {
  location: L;
  context: SlotContexts[L];
}) {
  const { t } = useLocale();
  const rights = useContext(StudioContext)?.access ?? [];
  return (
    <>
      {studioRegistry.contributions(location, rights).map((contribution) => (
        <WorkspaceBoundary
          key={contribution.id}
          title={t("failure")}
          retryLabel={t("refresh")}
        >
          <Suspense fallback={null}>
            <contribution.component context={context} />
          </Suspense>
        </WorkspaceBoundary>
      ))}
      {appLocations.includes(location) && (
        <AppSurfaceSlot location={location} context={context} />
      )}
    </>
  );
}
