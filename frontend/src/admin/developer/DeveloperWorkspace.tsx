/** Dedicated app and API workspaces retain App Studio state while switching the developer console. */
import { lazy, Suspense, useState } from "react";
import type { ComponentProps } from "react";
import DeveloperView from "./DeveloperView";
import IntegrationKeys from "./api/IntegrationKeys";
import AgentControls from "./api/AgentControls";
import ApiExplorer from "./api/ApiExplorer";
import { useApiText } from "./api/api-i18n";
import { useCatalogueText } from "./catalogue/catalogue-i18n";
const UICatalogue = lazy(() => import("./catalogue/UICatalogue"));
import "./api-console.css";
export default function DeveloperWorkspace(
  props: ComponentProps<typeof DeveloperView> & { workspace: string },
) {
  const t = useApiText(),
    c = useCatalogueText(),
    [tab, setTab] = useState<"studio" | "console" | "components">(
      props.rights && !props.rights.includes("developer.read")
        ? "console"
        : "studio",
    );
  const [homeRevision, setHomeRevision] = useState(0);
  const tabs = (
    props.rights && !props.rights.includes("developer.read")
      ? ["console"]
      : ["studio", "console", "components"]
  ) as ("studio" | "console" | "components")[];
  return (
    <>
      <nav className="developer-workspace-tabs workbench-row">
        {tabs.map((k) => (
          <button
            key={k}
            className={tab === k ? "studio-primary" : "studio-secondary"}
            aria-pressed={tab === k}
            onClick={() => {
              setTab(k);
              if (k === "studio") setHomeRevision((v) => v + 1);
            }}
          >
            {k === "components" ? c("tab") : t(k)}
          </button>
        ))}
      </nav>
      <div hidden={tab !== "studio"}>
        {tabs.includes("studio") && (
          <DeveloperView {...props} homeRevision={homeRevision} />
        )}
      </div>
      {tab === "console" && (
        <div className="studio-page api-console">
          <div className="page-intro">
            <h1>{t("console")}</h1>
            <p>{t("intro")}</p>
          </div>
          <IntegrationKeys request={props.request} />
          <AgentControls request={props.request} />
          <ApiExplorer request={props.request} workspace={props.workspace} />
        </div>
      )}
      {tab === "components" && (
        <Suspense fallback={<div role="status">…</div>}>
          <UICatalogue />
        </Suspense>
      )}
    </>
  );
}
