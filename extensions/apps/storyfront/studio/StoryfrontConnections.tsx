/** Shared passive navigation from Apps and Storyfronts to the same authorized original editor. */
import { Card, LinkButton } from "../../../../frontend/src/kit/ui";
import Icon from "../../../../frontend/src/shared/ui/Icon";
import { useLocale } from "../../../../frontend/src/shared/i18n/i18n";
import { useStoryfrontText } from "./storyfront-i18n";
import { connectionUrl, type FrontendConnection } from "./storyfront-model";
import "./storyfronts.css";
export default function StoryfrontConnections({
  frontends,
}: {
  frontends: FrontendConnection[];
}) {
  const { locale } = useLocale(),
    text = useStoryfrontText();
  return (
    <div className="storyfront-grid">
      {frontends.map((f) => {
        const url = connectionUrl(f.url),
          editor = connectionUrl(f.editorUrl, locale);
        return (
          <Card as="article" className="storyfront-card" key={f.alias}>
            <div className="storyfront-card-cover">
              <Icon name="layers" />
              <span>{text("connected")}</span>
            </div>
            <div className="storyfront-card-body">
              <h2>{f.alias}</h2>
              <p className="storyfront-address">
                {url ? new URL(url).hostname : f.alias}
              </p>
              <dl>
                <dt>{text("channel")}</dt>
                <dd>{f.channel}</dd>
              </dl>
              <div className="storyfront-card-actions">
                {editor ? (
                  <LinkButton
                    tone="primary"
                    href={editor}
                    target="_blank"
                    rel="noopener noreferrer"
                  >
                    <Icon name="settings" />
                    {text("edit")}
                  </LinkButton>
                ) : (
                  <p>{text("editorMissing")}</p>
                )}
                {url && (
                  <LinkButton
                    href={url}
                    target="_blank"
                    rel="noopener noreferrer"
                  >
                    <Icon name="arrow" />
                    {text("open")}
                  </LinkButton>
                )}
              </div>
            </div>
          </Card>
        );
      })}
    </div>
  );
}
