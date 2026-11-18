/** The latest saved attack test of the sandbox beside the release review; evidence only, never an approval gate. */
import type { AttackSummary } from "../scenarios/scenario-types";
import { useAttackText } from "../scenarios/scenario-attack-i18n";
import { useLocale } from "../../shared/i18n/i18n";
import "../scenarios/scenario-labs.css";
export default function ReleaseAttack({
  attack,
}: {
  attack?: AttackSummary | null;
}) {
  const t = useAttackText(),
    { locale } = useLocale();
  const clean =
    !!attack && attack.summary.breached + attack.summary.review === 0;
  return (
    <div className="release-attack">
      <h3>{t("attackReleaseTitle")}</h3>
      {!attack ? (
        <p>{t("attackReleaseNone")}</p>
      ) : (
        <>
          <p
            className={`scenario-attack-summary${clean ? "" : " is-breached"}`}
          >
            <strong>
              {t("attackSummary")
                .replace("{held}", String(attack.summary.held))
                .replace("{total}", String(attack.total))}
            </strong>
          </p>
          <p className="scenario-attack-detail">
            {attack.savedAt &&
              !Number.isNaN(Date.parse(attack.savedAt)) &&
              `${t("attackSaved").replace("{time}", new Date(attack.savedAt).toLocaleString(locale))} `}
            {t(
              attack.configurationReview === "current"
                ? "attackCurrent"
                : attack.configurationReview === "stale" ||
                    attack.configurationReview === "changed_during_run"
                  ? "attackStale"
                  : "attackUnverified",
            )}
          </p>
        </>
      )}
    </div>
  );
}
