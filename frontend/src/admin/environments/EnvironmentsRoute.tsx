/** Environments route: sandbox stages of the live shop, with scoped requests, test scenarios and stage selection. */
import { useCallback } from "react";
import EnvironmentManager from "./EnvironmentManager";
import { useScenarioNavigation } from "../scenarios/useScenarioNavigation";
import { createStudioRequest } from "../shell/requests";
import { useLocale } from "../../shared/i18n/i18n";
import { useStudio } from "../shell/StudioContext";

export default function EnvironmentsRoute() {
  const {
    token,
    workspace,
    environment,
    liveRequest,
    environments,
    refreshEnvironments,
    role,
    setEnvironment,
    access,
  } = useStudio();
  const { locale } = useLocale();
  const openScenarios = useScenarioNavigation();
  const sandboxRequest = useCallback(
    (id: string) => createStudioRequest(token, id, locale),
    [token, locale],
  );
  return (
    <EnvironmentManager
      key={workspace}
      sandboxRequest={sandboxRequest}
      onTest={(target, channel, scenario) =>
        openScenarios(channel, target, undefined, scenario)
      }
      initialId={environment}
      request={liveRequest}
      environments={environments}
      onRefresh={refreshEnvironments}
      role={role}
      onSelect={setEnvironment}
      rights={access}
    />
  );
}
