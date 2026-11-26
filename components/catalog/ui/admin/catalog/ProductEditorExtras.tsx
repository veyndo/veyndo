/** Permission-filtered extension slots retain the current product context. */
import { forApps } from "../../../../../frontend/src/shared/api/types";
import CoreExtensions, {
  type CoreExtensionData,
} from "../../../../../frontend/src/shared/apps/CoreExtensions";
import { AppSurfaceSlot } from "../../../../../frontend/src/shared/apps/AppSurfaces";
import StudioSlot from "../../../../../frontend/src/admin/shell/registry/StudioSlot";
import type { RequestFn } from "../../../../../frontend/src/admin/shell/studio-types";
export default function ProductEditorExtras({
  extensions,
  id,
  tab,
  request,
  selectedApp,
}: {
  extensions?: CoreExtensionData;
  id: string;
  tab: string;
  request: RequestFn;
  selectedApp: boolean;
}) {
  return (
    <>
      {id && tab === "general" && (
        <CoreExtensions
          objectId={id}
          {...extensions}
          request={forApps(request)}
        />
      )}
      {id && tab === "general" && (
        <StudioSlot
          location="admin.product.general"
          context={{ productId: id }}
        />
      )}
      {id && !selectedApp && (
        <AppSurfaceSlot location="admin.product" context={{ productId: id }} />
      )}
    </>
  );
}
