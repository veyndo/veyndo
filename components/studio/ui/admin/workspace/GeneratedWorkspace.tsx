/** Render the admitted workspace layout through the shared catalog renderer; the native controls are host components. */
import { isValidElement, useMemo, type ReactNode } from "react";
import {
  CatalogView,
  type HostComponent,
} from "../../../../../frontend/src/kit/catalog";
import { useLocale } from "../../../../../frontend/src/shared/i18n/i18n";
import {
  workspaceCatalogId,
  workspaceDocument,
} from "./workspace-presentation";

const slot = (description: string): HostComponent => ({
  description,
  render: () => null,
});

export default function GeneratedWorkspace({
  candidate,
  rights,
  directory,
  intake,
  shippingPolicy,
}: {
  candidate: unknown;
  rights: string[];
  directory: ReactNode;
  intake: ReactNode;
  shippingPolicy?: ReactNode;
}) {
  const { locale } = useLocale();
  const presentation = useMemo(
    () => workspaceDocument(candidate, rights),
    [candidate, rights],
  );
  const host = useMemo<Record<string, HostComponent>>(
    () => ({
      Workspace: {
        description: "A reviewed native Veyndo working view.",
        slots: {
          children: {
            description: "The admitted native controls.",
            accepts: ["Directory", "Intake", "ShippingPolicy"],
            max: 3,
          },
        },
        render: ({ slots }) => (
          <div className="generated-native-workspace">
            {slots.children.map((child, index) => (
              <div key={isValidElement(child) ? child.key : index}>{child}</div>
            ))}
          </div>
        ),
      },
      Directory: {
        ...slot("Current authorized native directory and editor."),
        render: () => directory,
      },
      Intake: {
        ...slot("Native reviewed catalogue intake, when authorized."),
        render: () => intake,
      },
      ShippingPolicy: {
        ...slot("Current shipping policy passages."),
        render: () => shippingPolicy,
      },
    }),
    [directory, intake, shippingPolicy],
  );
  const admission = useMemo(
    () => ({
      surface: "studio" as const,
      locale,
      host,
      catalogs: [workspaceCatalogId],
      openUiCatalog: workspaceCatalogId,
    }),
    [locale, host],
  );
  if (!presentation) return null;
  return (
    <div data-layout-protocol={presentation.protocol}>
      <CatalogView document={presentation.document} admission={admission} />
    </div>
  );
}
