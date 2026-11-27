/** Public service directory. Links select a login surface on the configured Studio origin without granting operator or merchant permissions. */
import Brand from "../shared/ui/Brand";
import { useControlText } from "../shared/i18n/control-i18n";
import PlatformLanguage from "./PlatformLanguage";
import { useStudioOrigin } from "../shared/api/studio-origin";
import "./styles/platform.css";
export default function AdminHub() {
  const t = useControlText();
  // Both sign-ins live on the configured Studio origin, which the server names when this
  // directory host does not admit browser sessions.
  const studio = useStudioOrigin();
  const shared = studio ? `${studio}/` : "/";
  return (
    <div className="platform-console platform-hub">
      <main>
        <header>
          <Brand />
          <PlatformLanguage />
        </header>
        <div className="platform-hub-intro">
          <h1>{t("hub")}</h1>
          <p>{t("hubHint")}</p>
        </div>
        <div className="platform-hub-links">
          <a href={`${shared}#login`}>
            <span aria-hidden="true">↗</span>
            <h2>{t("merchant")}</h2>
            <p>{t("merchantHint")}</p>
          </a>
          <a href={studio ? `${studio}/#platform` : "#platform"}>
            <span aria-hidden="true">◎</span>
            <h2>{t("operator")}</h2>
            <p>{t("operatorHint")}</p>
          </a>
          <a
            href="https://sthamann.github.io/veyndo/docs/"
            target="_blank"
            rel="noreferrer"
          >
            <span aria-hidden="true">⌘</span>
            <h2>{t("documentation")}</h2>
          </a>
        </div>
      </main>
    </div>
  );
}
