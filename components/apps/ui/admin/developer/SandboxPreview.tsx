/** Preview resolves the installed registry first; a newer staged package cannot masquerade as an older build. */
import { forApps } from "../../../../../frontend/src/shared/api/types";
import { useRead } from "../../../../../frontend/src/admin/shell/useRead";
import { Notice } from "../../../../../frontend/src/kit/ui";
import { useEffect, useState } from "react";
import SandboxContextPicker from "./SandboxContextPicker";
import useSurfaceGateway from "../../../../../frontend/src/shared/apps/useSurfaceGateway";
import NativeAppView from "../../../../../frontend/src/shared/apps/native/NativeAppView";
import type { AppSurface } from "../../../../../frontend/src/shared/apps/AppSurfaces";
import type { RequestFn } from "../../../../../frontend/src/admin/shell/studio-types";
import { useAppStudioText } from "../../../../../frontend/src/shared/i18n/app-studio-i18n";
export default function SandboxPreview({
  app,
  version,
  view,
  request,
  onLanguages,
}: {
  app: string;
  version: string;
  view: string;
  request: RequestFn;
  onLanguages?: (mainLocale: string, locales: string[]) => void;
}) {
  const { a } = useAppStudioText();
  const [context, setContext] = useState<Record<string, unknown>>({});
  const admin = useRead(request, "/api/apps/surfaces");
  const store = useRead(request, "/store-api/apps/surfaces");
  const loaded = !!admin.data && !!store.data;
  const surface: AppSurface | null = loaded
    ? ([...admin.data.surfaces, ...store.data.surfaces].find(
        (s: AppSurface) =>
          s.app === app &&
          s.version === version &&
          s.surface.id === view &&
          !!s.native,
      ) ?? null)
    : null;
  const error =
    (admin.error as Error | undefined)?.message ??
    (store.error as Error | undefined)?.message ??
    (loaded && !surface ? a("notStaged") : "");
  useEffect(() => setContext({}), [request, app, version, view]);
  // The staged view's languages are reported once it is found.
  useEffect(() => {
    if (surface)
      onLanguages?.(
        surface.mainLocale ?? "en-GB",
        surface.locales ?? ["en-GB"],
      );
  }, [surface?.app, surface?.version, surface?.surface.id]);
  const scoped = useSurfaceGateway(
    forApps(request),
    app,
    view,
    context,
    !!surface && !surface.surface.location.startsWith("admin."),
  );
  return (
    <div className="app-sandbox-preview">
      <div className="app-canvas-label">
        <span>{a("realPreview")}</span>
        <span>{version}</span>
      </div>
      {error ? (
        <Notice tone="danger">{error}</Notice>
      ) : surface?.native ? (
        <>
          {surface.native.view.blocks.find((b) => b.contextBinding)
            ?.contextBinding && (
            <SandboxContextPicker
              key={`${app}:${view}`}
              binding={
                surface.native.view.blocks.find((b) => b.contextBinding)!
                  .contextBinding!
              }
              request={request}
              mainLocale={surface.mainLocale ?? "en-GB"}
              onSelect={setContext}
            />
          )}
          <NativeAppView
            key={JSON.stringify(context)}
            context={context}
            app={app}
            native={surface.native}
            request={scoped}
            debug
            allowedActions={surface.surface.actions}
            mainLocale={surface.mainLocale}
            locales={surface.locales}
            inheritContentLanguage
            public={!surface.surface.location.startsWith("admin.")}
          />
        </>
      ) : (
        <p>{a("loading")}</p>
      )}
    </div>
  );
}
