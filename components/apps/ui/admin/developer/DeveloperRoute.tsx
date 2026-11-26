/** Developer route: the developer workspace on the live shop, with sandbox-scoped requests and test scenarios. */
import { useCallback } from "react";
import DeveloperWorkspace from "./DeveloperWorkspace";
import { useScenarioNavigation } from "../../../../../frontend/src/admin/shell/useScenarioNavigation";
import { createStudioRequest } from "../../../../../frontend/src/admin/shell/requests";
import { useLocale } from "../../../../../frontend/src/shared/i18n/i18n";
import { useStudio } from "../../../../../frontend/src/admin/shell/StudioContext";

export default function DeveloperRoute() {
  const {
    token,
    workspace,
    refreshEnvironments,
    selectTab,
    liveRequest,
    environments,
    role,
    access,
  } = useStudio();
  const { locale } = useLocale();
  const openScenarios = useScenarioNavigation();
  const sandboxRequest = useCallback(
    (id: string) => createStudioRequest(token, id, locale),
    [token, locale],
  );
  return (
    <DeveloperWorkspace
      onTest={(target) => openScenarios("default", target)}
      key={workspace}
      workspace={workspace}
      sandboxRequest={sandboxRequest}
      onRefresh={refreshEnvironments}
      onFlows={() => selectTab("automation")}
      request={liveRequest}
      environments={environments}
      role={role}
      rights={access}
    />
  );
}
