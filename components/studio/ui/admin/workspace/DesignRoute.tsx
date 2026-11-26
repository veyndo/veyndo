/** Design route: opens the site builder designer with the design reference that belongs to the current shop and stage. */
import { forApps } from "../../../../../frontend/src/shared/api/types";
import { lazy } from "react";
import { useScenarioNavigation } from "../../../../../frontend/src/admin/shell/useScenarioNavigation";
import { useStudio } from "../../../../../frontend/src/admin/shell/StudioContext";

const ShopDesigner = lazy(
  () =>
    import("../../../../../extensions/apps/site-builder/studio/ShopDesigner"),
);

export default function DesignRoute() {
  const {
    request,
    environment,
    workspace,
    workspaceName,
    access,
    designReference,
    selectTab,
    refreshEnvironments,
    setEnvironment,
  } = useStudio();
  const openScenarios = useScenarioNavigation();
  return (
    <ShopDesigner
      request={forApps(request)}
      workspace={environment || workspace}
      shopName={workspaceName}
      rights={access}
      sandbox={!!environment}
      initialReference={
        designReference?.workspace === workspace &&
        designReference.environment === (environment || undefined)
          ? designReference.url
          : undefined
      }
      onEnvironments={() => selectTab("environments")}
      onTest={(channel) => void openScenarios(channel)}
      onSandbox={async (id, current) => {
        await refreshEnvironments();
        if (current?.() !== false) setEnvironment(id);
      }}
    />
  );
}
