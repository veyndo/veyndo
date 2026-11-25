/** Channels route: sales channels with stage-scoped requests, test scenarios and the hand-off to channel design. */
import { useCallback } from "react";
import SalesChannelsWorkspace from "./SalesChannelsWorkspace";
import { useScenarioNavigation } from "../../../../../frontend/src/admin/shell/useScenarioNavigation";
import { createStudioRequest } from "../../../../../frontend/src/admin/shell/requests";
import { useLocale } from "../../../../../frontend/src/shared/i18n/i18n";
import { useStudio } from "../../../../../frontend/src/admin/shell/StudioContext";

export default function ChannelsRoute() {
  const {
    token,
    environment,
    workspace,
    request,
    selectTab,
    refreshEnvironments,
    setEnvironment,
    setDesignReference,
  } = useStudio();
  const { locale } = useLocale();
  const openScenarios = useScenarioNavigation();
  const sandboxRequest = useCallback(
    (id: string) => createStudioRequest(token, id, locale),
    [token, locale],
  );
  return (
    <SalesChannelsWorkspace
      onTest={(target) => openScenarios("default", target)}
      scopedRequest={sandboxRequest}
      key={environment || workspace}
      request={request}
      workspace={environment || workspace}
      onTeam={() => selectTab("users")}
      sandbox={!!environment}
      onDesign={async (id, target, reference) => {
        if (target) {
          await refreshEnvironments();
          setEnvironment(target);
        }
        setDesignReference(
          reference
            ? {
                workspace,
                environment: target || environment || undefined,
                url: reference,
              }
            : undefined,
        );
        const url = new URL(location.href);
        url.searchParams.set("designChannel", id);
        history.replaceState(null, "", url);
        selectTab("design");
      }}
    />
  );
}
