/** Settings route: shop settings with links to connections, team, automation, B2B, channels and apps. */
import SettingsWorkspace from "./SettingsWorkspace";
import { useStudio } from "../shell/StudioContext";

export default function SettingsRoute() {
  const { request, access, setSettings, selectTab } = useStudio();
  return (
    <SettingsWorkspace
      request={request}
      rights={access}
      onConnections={() => setSettings(true)}
      onTeam={() => selectTab("users")}
      onAutomation={() => selectTab("automation")}
      onB2B={() => selectTab("b2b")}
      onChannels={() => selectTab("channels")}
      onApps={() => selectTab("apps")}
    />
  );
}
