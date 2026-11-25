/** Channel settings embed existing revision-aware identity and checkout editors with the channel selected. */
import { Button } from "../../../../../frontend/src/kit/ui";
import { useCurrencyText } from "../../../../../frontend/src/shared/i18n/currency-i18n";
import { useState } from "react";
import type { RequestFn } from "../../../../../frontend/src/admin/shell/studio-types";
import LegalSettings from "../../../../shop/ui/admin/legal/LegalSettings";
import { useLegalText } from "../../../../../frontend/src/shared/i18n/legal-i18n";
import MasterDataSettings from "../../../../shop/ui/admin/settings/MasterDataSettings";
import CommerceSettings from "../../../../shop/ui/admin/settings/CommerceSettings";
import { useChannelText } from "./channel-i18n";
export default function ChannelSettings({
  request,
  channel,
  canWrite,
  onDirty,
}: {
  request: RequestFn;
  channel: string;
  canWrite: boolean;
  onDirty: (dirty: boolean) => void;
}) {
  const { c: ct } = useCurrencyText();
  const { l } = useLegalText();
  const t = useChannelText(),
    [area, setArea] = useState<
      "company" | "taxes" | "shipping" | "payment" | "legal" | "currencies"
    >("company"),
    [dirty, setDirty] = useState(false);
  const notify = (value: boolean) => {
    setDirty(value);
    onDirty(value);
  };
  return (
    <>
      <p>{t("scopeHint")}</p>
      <nav className="workbench-row">
        {(
          [
            "company",
            "currencies",
            "taxes",
            "shipping",
            "payment",
            "legal",
          ] as const
        ).map((k) => (
          <Button
            tone={k === area ? "primary" : "secondary"}
            type="submit"
            key={k}
            disabled={dirty && k !== area}
            onClick={() => setArea(k)}
          >
            {k === "legal"
              ? l("title")
              : k === "currencies"
                ? ct("title")
                : t(k)}
          </Button>
        ))}
      </nav>
      {area === "company" ? (
        <MasterDataSettings
          key={channel + area}
          request={request}
          canWrite={canWrite}
          initialChannel={channel}
          onDirty={notify}
        />
      ) : area === "legal" ? (
        <LegalSettings
          key={channel + area}
          request={request}
          canWrite={canWrite}
          initialChannel={channel}
          onDirty={notify}
        />
      ) : (
        <CommerceSettings
          key={channel + area}
          request={request}
          canWrite={canWrite}
          initialChannel={channel}
          area={area}
          onDirty={notify}
        />
      )}
    </>
  );
}
