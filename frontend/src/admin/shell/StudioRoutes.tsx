/** StudioRoutes: renders the registered route of the selected Studio area after the shell's connection, rights and data guards. */
import { Button } from "../../kit/ui";
import "../../shared/styles/workbench.css";
import Icon from "../../shared/ui/Icon";
import "../styles/operations.css";
import "../styles/studio.css";
import { canOpenStudio } from "./permissions";
import { usePermissionText } from "../../../../components/identity/ui/admin/team/permission-i18n";
import StudioCommerceRoute from "./StudioCommerceRoute";
import { useStudio } from "./StudioContext";
import { studioRegistry } from "./registry/first-party";
export default function StudioRoutes() {
  const {
    auth,
    appSurface,
    tab,
    environment,
    setEnvironment,
    connected,
    connectionPending,
    retryConnection,
    data,
    access,
    selectTab,
    nav,
    tabLabel,
    t,
    w,
    x,
  } = useStudio();
  const pt = usePermissionText();
  // Keep the conversation draft mounted through session recovery; data workspaces wait for their new scope.
  if (!connected && auth.active)
    return (
      <div
        className="studio-empty"
        role={connectionPending ? "status" : "alert"}
      >
        <Icon name={connectionPending ? "refresh" : "link"} size={32} />
        <p>
          {x(connectionPending ? "loadingWorkspace" : "workspaceLoadFailed")}
        </p>
        {!connectionPending && (
          <>
            <Button tone="primary" type="submit" onClick={retryConnection}>
              {t("refresh")}
            </Button>
            {environment && (
              <Button type="submit" onClick={() => setEnvironment("")}>
                {w("live")}
              </Button>
            )}
          </>
        )}
      </div>
    );
  if (!appSurface && !canOpenStudio(tab, access))
    return (
      <div className="studio-empty" role="alert">
        <Icon name="lock" size={32} />
        <p>{pt("noAccess")}</p>
      </div>
    );
  if (appSurface) return <StudioCommerceRoute />;
  const route = studioRegistry.route(tab)!;
  if (route.requires === "connection" && !connected)
    return (
      <div className="studio-empty">
        <p>{t("connectFirst")}</p>
        <Button tone="primary" type="submit" onClick={() => selectTab("users")}>
          {t("users")}
        </Button>
      </div>
    );
  if (route.requires === "data" && !data)
    return (
      <div className="studio-empty">
        <Icon name={nav.find((n) => n.id === tab)!.icon} size={48} />
        <h1>{tabLabel(tab)}</h1>
        <p>{t("connectFirst")}</p>
        <Button tone="primary" type="submit" onClick={() => selectTab("users")}>
          {x("studioSignIn")}
          <Icon name="link" size={18} />
        </Button>
      </div>
    );
  return <route.component />;
}
