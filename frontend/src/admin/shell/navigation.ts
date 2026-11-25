/** Typed Studio route ids, their locale-specific labels and the navigation entries the extensions register. */
import { useSetupText } from "../setup/setup-i18n";
import { useB2BExperienceText } from "../b2b/experience-i18n";
import { useB2BText } from "../../shared/i18n/b2b-i18n";
import { usePosText } from "../pos/pos-i18n";
import { useLocale } from "../../shared/i18n/i18n";
import { useOperationsText } from "../../shared/i18n/operations-i18n";
import { useWorkbenchText } from "../../shared/i18n/workbench-i18n";
import { type IconName } from "../../shared/ui/Icon";
import { useWorkspaceText } from "../workspace/workspace-i18n";
import { studioRegistry } from "./registry/first-party";
export type Tab =
  | "setup"
  | "workspace"
  | "design"
  | "assistant"
  | "overview"
  | "knowledge"
  | "agents"
  | "commerce"
  | "users"
  | "apps"
  | "storyfronts"
  | "developers"
  | "environments"
  | "automation"
  | "productData"
  | "orders"
  | "customers"
  | "channels"
  | "pos"
  | "b2b"
  | "b2bQuotes";

export function useStudioNavigation() {
  const { t } = useLocale();
  const setup = useSetupText();
  const personal = useWorkspaceText();
  const p = usePosText();
  const b = useB2BText(),
    bx = useB2BExperienceText();
  const { w } = useWorkbenchText();
  const { o } = useOperationsText();
  const tabLabel = (id: Tab): string => {
    if (id === "setup") return setup("title");
    if (id === "workspace") return personal("title");
    if (id === "design") return personal("design");
    if (id === "b2b") return b("title");
    if (id === "b2bQuotes") return bx("quotes");
    if (id === "pos") return p("title");
    if (id === "channels") return w("channels");
    if (id === "commerce") return t("settings");
    if (id === "orders" || id === "customers") return o(id);
    if (
      id === "storyfronts" ||
      id === "developers" ||
      id === "environments" ||
      id === "automation" ||
      id === "productData"
    )
      return w(id);
    return t(id);
  };

  const nav: { id: Tab; icon: IconName }[] = studioRegistry.routes.map(
    ({ id, icon }) => ({ id, icon }),
  );

  return { tabLabel, nav };
}
