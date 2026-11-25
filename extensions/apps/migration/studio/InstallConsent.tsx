/** Reuse native package consent for migration and its private page-editor destination. */
import AppConsent from "../../../../components/apps/ui/admin/apps/AppConsent";
import type { RequestFn } from "../../../../frontend/src/shared/api/types";
export default function InstallConsent({
  app,
  request,
  onCancel,
  onInstalled,
}: {
  app: string;
  request: RequestFn;
  onCancel: () => void;
  onInstalled: () => Promise<void>;
}) {
  return (
    <AppConsent
      app={app}
      request={request}
      onCancel={onCancel}
      onInstall={async (consent) => {
        await request("/api/apps", {
          builtIn: app,
          approve: true,
          digest: consent.digest,
          permissions: consent.permissions,
        });
        await onInstalled();
        dispatchEvent(new Event("commerce.apps.changed"));
      }}
    />
  );
}
