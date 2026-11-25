/** Review the actual package and permission changes before installation; consent is enforced by the API. */
import { Badge, Notice } from "../../../../../frontend/src/kit/ui";
import { useState } from "react";
import type { RequestFn } from "../../../../../frontend/src/admin/shell/studio-types";
import { useRead } from "../../../../../frontend/src/admin/shell/useRead";
import ConfirmDialog from "../../../../../frontend/src/shared/ui/ConfirmDialog";
import { useAppAccessText } from "../../../../../frontend/src/shared/i18n/app-access-i18n";
import { contentText } from "../../../../../frontend/src/shared/i18n/content-language";
import { useAppText } from "../../../../../frontend/src/shared/i18n/app-i18n";
import AppUpgradeReport, { type UpgradeReport } from "./AppUpgradeReport";
type Review = {
  app: string;
  name: Record<string, string>;
  version: string;
  digest: string;
  permissions: string[];
  added: string[];
  previousVersion: string | null;
  upgrade?: UpgradeReport;
};
export default function AppConsent({
  app,
  request,
  manifest,
  onCancel,
  onInstall,
}: {
  app: string;
  request: RequestFn;
  manifest?: Record<string, unknown>;
  onCancel: () => void;
  onInstall: (
    consent: Review & { approve: true; builtIn: string },
  ) => Promise<void>;
}) {
  const t = useAppAccessText(),
    { a, locale } = useAppText();
  const [actionError, setError] = useState(""),
    [busy, setBusy] = useState(false);
  // The review is a read exposed as POST; its key carries the app or manifest.
  const reviewRead = useRead<Review>(request, "/api/apps/review", {
    body: manifest ? { manifest } : { builtIn: app },
  });
  const review = reviewRead.data ?? null;
  const error =
    actionError || (reviewRead.error as Error | undefined)?.message || "";
  return (
    <ConfirmDialog
      title={t("installConsent")}
      confirmLabel={a(review?.previousVersion ? "upgrade" : "install")}
      disabled={!review || busy || review.upgrade?.ready === false}
      onCancel={onCancel}
      onConfirm={async () => {
        if (!review || review.upgrade?.ready === false) return;
        setBusy(true);
        setError("");
        try {
          await onInstall({ ...review, approve: true, builtIn: app });
        } catch (e) {
          setError((e as Error).message);
        } finally {
          setBusy(false);
        }
      }}
    >
      <p>{t("installHint")}</p>
      {review && (
        <>
          <strong>
            {contentText(review.name, locale, "en-GB")} · {review.version}
          </strong>
          <code className="app-key-secret">{review.digest}</code>
          {review.upgrade && <AppUpgradeReport report={review.upgrade} />}
          <ul>
            {review.permissions.map((p) => (
              <li key={p}>
                <code>{p}</code>
                {review.added.includes(p) && <Badge>{t("added")}</Badge>}
              </li>
            ))}
          </ul>
        </>
      )}
      {error && <Notice tone="danger">{error}</Notice>}
    </ConfirmDialog>
  );
}
