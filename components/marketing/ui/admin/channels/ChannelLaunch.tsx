/** Successful native channel creation ends in a clear next action; no provider is connected implicitly. */
import { Button, Card } from "../../../../../frontend/src/kit/ui";
import Icon from "../../../../../frontend/src/shared/ui/Icon";
import type { Channel } from "./channel-model";
import { useChannelText } from "./channel-i18n";
import { contentText } from "../../../../../frontend/src/shared/i18n/content-language";
import { useLocale } from "../../../../../frontend/src/shared/i18n/i18n";
export default function ChannelLaunch({
  channel,
  mainLocale,
  onEdit,
  onMarketplace,
}: {
  channel: Channel;
  mainLocale: string;
  onEdit: () => void;
  onMarketplace?: () => void;
}) {
  const t = useChannelText(),
    { locale } = useLocale();
  return (
    <Card className="channel-launch">
      <span className="channel-avatar">
        <Icon name="check" size={30} />
      </span>
      <p role="status">{t("saved")}</p>
      <h2>{contentText(channel.data.name, locale, mainLocale)}</h2>
      <p>{t("createdHint")}</p>
      <div className="workbench-row">
        {channel.data.kind === "marketplace" && onMarketplace && (
          <Button tone="primary" type="submit" onClick={onMarketplace}>
            {t("marketplaceManage")} <Icon name="arrow" size={18} />
          </Button>
        )}
        <Button type="submit" onClick={onEdit}>
          {t("edit")}
        </Button>
      </div>
    </Card>
  );
}
