/** Shared customer-test entry; installing/saving a draft remains the owning workspace's action. */
import { Button } from "../../../../../frontend/src/kit/ui";
import Icon from "../../../../../frontend/src/shared/ui/Icon";
import { canTestCustomers } from "./scenario-access";
import { useScenarioText } from "./scenario-i18n";
export default function ScenarioLaunch({
  rights,
  disabled,
  onClick,
}: {
  rights: string[];
  disabled?: boolean;
  onClick: () => void;
}) {
  const t = useScenarioText();
  return canTestCustomers(rights) ? (
    <Button type="submit" disabled={disabled} onClick={onClick}>
      <Icon name="people" />
      {t("title")}
    </Button>
  ) : null;
}
