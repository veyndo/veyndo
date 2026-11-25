/** Saved app cards with explicit editing and recoverable project removal, independent of installed package/data lifecycle. */
import { Button } from "../../../../../frontend/src/kit/ui";
import { useState } from "react";
import { contentText } from "../../../../../frontend/src/shared/i18n/content-language";
import { useAppStudioText } from "../../../../../frontend/src/shared/i18n/app-studio-i18n";
import Icon from "../../../../../frontend/src/shared/ui/Icon";
import type { Build } from "./useAppStudio";
const latest = (builds: Build[]) =>
  builds.filter(
    (b, index) => builds.findIndex((v) => v.app === b.app) === index,
  );
export default function AppLibrary({
  builds,
  archived,
  mainLocale,
  busy,
  onOpen,
  onArchive,
}: {
  builds: Build[];
  archived: Build[];
  mainLocale: string;
  busy: boolean;
  onOpen: (build: Build) => void;
  onArchive: (app: string, archived: boolean) => void;
}) {
  const { a, locale } = useAppStudioText();
  const [removing, setRemoving] = useState<Build | null>(null);
  const name = (b: Build) =>
    contentText(b.manifest.name, locale, mainLocale) || b.app;
  if (!builds.length && !archived.length) return null;
  return (
    <section className="app-library" aria-label={a("library")}>
      <h2>{a("library")}</h2>
      <div>
        {latest(builds).map((build) => (
          <article key={build.app} className="app-library-card">
            <button
              className="app-library-open"
              disabled={busy}
              onClick={() => onOpen(build)}
            >
              <Icon name="layers" size={22} />
              <span>
                <strong>{name(build)}</strong>
                <small>
                  {build.app} · {build.version}
                </small>
                <span>{a("openApp")}</span>
              </span>
              <Icon name="arrow" size={16} />
            </button>
            <footer>
              <Button
                type="submit"
                disabled={busy}
                aria-label={`${a("editApp")}: ${name(build)}`}
                onClick={() => onOpen(build)}
              >
                {a("editApp")}
              </Button>
              <Button
                type="submit"
                disabled={busy}
                aria-label={`${a("deleteApp")}: ${name(build)}`}
                onClick={() => setRemoving(build)}
              >
                {a("deleteApp")}
              </Button>
            </footer>
          </article>
        ))}
      </div>
      {removing && (
        <section
          className="app-library-confirm"
          role="alertdialog"
          aria-labelledby="app-trash-title"
          aria-describedby="app-trash-hint"
        >
          <h3 id="app-trash-title">
            {a("confirmDelete")}: {name(removing)}
          </h3>
          <p id="app-trash-hint">{a("deleteHint")}</p>
          <Button type="submit" autoFocus onClick={() => setRemoving(null)}>
            {a("cancel")}
          </Button>
          <Button
            tone="primary"
            type="submit"
            disabled={busy}
            onClick={() => {
              onArchive(removing.app, true);
              setRemoving(null);
            }}
          >
            {a("confirmDelete")}
          </Button>
        </section>
      )}
      {!!archived.length && (
        <details className="app-library-trash">
          <summary>
            {a("trash")} · {latest(archived).length}
          </summary>
          {latest(archived).map((build) => (
            <div key={build.app}>
              <strong>{name(build)}</strong>
              <code>{build.app}</code>
              <Button
                type="submit"
                disabled={busy}
                aria-label={`${a("restoreApp")}: ${name(build)}`}
                onClick={() => onArchive(build.app, false)}
              >
                {a("restoreApp")}
              </Button>
            </div>
          ))}
        </details>
      )}
    </section>
  );
}
