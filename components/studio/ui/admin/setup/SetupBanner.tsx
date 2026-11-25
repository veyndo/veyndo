/** Keeps the setup return path visible while the merchant works in the actual native views. */
import { Button } from "../../../../../frontend/src/kit/ui";
import { useStudio } from "../../../../../frontend/src/admin/shell/StudioContext";
import { useShopSetup } from "./SetupProvider";
import { useSetupText } from "./setup-i18n";
import { useStepText } from "./step-i18n";
import Icon from "../../../../../frontend/src/shared/ui/Icon";
import "./setup.css";
import "./setup-responsive.css";
export default function SetupBanner() {
  const { tab, selectTab } = useStudio();
  const guide = useShopSetup(),
    t = useSetupText(),
    stepText = useStepText();
  if (!guide.available || guide.progress?.state !== "active" || tab === "setup")
    return null;
  return (
    <aside className="setup-return" aria-label={t("continuing")}>
      <Icon name="check" size={18} />
      <span>
        {t("continuing")}
        <strong>{stepText(guide.progress.step).title}</strong>
      </span>
      <Button
        type="submit"
        onClick={() => {
          selectTab("setup");
        }}
      >
        {t("return")}
        <Icon name="arrow" size={16} />
      </Button>
    </aside>
  );
}
