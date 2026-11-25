/** App Studio heading and new-app entry share the localized workbench language. */
import { Button } from "../../kit/ui";
import { usePermissionText } from "../team/permission-i18n";
import Icon from "../../shared/ui/Icon";
import { useAppStudioText } from "../../shared/i18n/app-studio-i18n";
export default function AppStudioIntro({
  disabled,
  onNew,
  overview,
  compact,
}: {
  disabled: boolean;
  onNew: () => void;
  overview?: boolean;
  compact?: boolean;
}) {
  const pt = usePermissionText();
  const { a } = useAppStudioText();
  if (compact)
    return (
      <button
        type="button"
        className="studio-secondary app-canvas-back"
        title={pt("back")}
        aria-label={pt("back")}
        onClick={onNew}
      >
        <Icon name="chevron-left" size={16} />
        {pt("back")}
      </button>
    );
  return (
    <div className="app-studio-intro">
      <div>
        <h1>{a("studio")}</h1>
        <p>{a("hint")}</p>
      </div>
      {overview && (
        <Button type="submit" onClick={onNew}>
          <Icon name="arrow" />
          {pt("back")}
        </Button>
      )}
      <Button tone="primary" type="submit" disabled={disabled} onClick={onNew}>
        <Icon name="plus" size={16} />
        {a("reset")}
      </Button>
    </div>
  );
}
