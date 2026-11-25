/** B2B route: companies or quotes in the current stage, or the disabled notice when the shop has not enabled them. */
import B2BWorkspace from "./B2BWorkspace";
import { useCommerceNavText } from "../shell/commerce-nav-i18n";
import { useStudio } from "../shell/StudioContext";

export default function B2BRoute() {
  const {
    b2bFeatures,
    tab,
    request,
    access,
    environment,
    workspace,
    selectTab,
  } = useStudio();
  const t = useCommerceNavText();
  if (!b2bFeatures) return null;
  if (!b2bFeatures.enabled || (tab === "b2bQuotes" && !b2bFeatures.quotes))
    return (
      <p className="studio-page" role="status">
        {t("b2bDisabled")}
      </p>
    );
  return (
    <B2BWorkspace
      key={`${environment || workspace}:${tab}`}
      quotes={tab === "b2bQuotes"}
      onQuotes={() => selectTab("b2bQuotes")}
      onCompanies={() => selectTab("b2b")}
      request={request}
      rights={access}
      onSettings={() => {
        const url = new URL(location.href);
        url.searchParams.set("settings", "b2b");
        history.replaceState(null, "", url);
        selectTab("commerce");
      }}
    />
  );
}
