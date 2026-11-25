/** Automation route: flows for the current stage, with links to channels and to a translation job in settings. */
import AutomationView from "./AutomationView";
import { useStudio } from "../shell/StudioContext";

export default function AutomationRoute() {
  const { environment, workspace, request, role, selectTab } = useStudio();
  return (
    <AutomationView
      key={environment || workspace}
      request={request}
      role={role}
      onChannels={() => selectTab("channels")}
      onTranslations={(jobId) => {
        const url = new URL(location.href);
        url.searchParams.set("settings", "languages");
        url.searchParams.set("translationJob", jobId);
        history.replaceState(null, "", url);
        selectTab("commerce");
      }}
    />
  );
}
