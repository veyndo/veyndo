/** SettingsDialog keeps merchant interaction separate from workspace orchestration. */
import { Button, Field, Select, TextInput } from "../../kit/ui";
import { useControlText } from "../../shared/i18n/control-i18n";
import { useEffect, useRef } from "react";
import { useLocale } from "../../shared/i18n/i18n";
import Icon from "../../shared/ui/Icon";
import type { Provider } from "../shell/studio-types";
export default function SettingsDialog({
  connected,
  providers,
  provider,
  model,
  onProvider,
  onModel,
  onClose,
  onIndex,
  busy,
}: {
  connected: boolean;
  providers: Provider[];
  provider: string;
  model: string;
  onProvider: (s: string) => void;
  onModel: (s: string) => void;
  onClose: () => void;
  onIndex: () => void;
  busy: boolean;
}) {
  const c = useControlText();
  const { t } = useLocale();
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    dialog.current?.showModal();
    return () => dialog.current?.close();
  }, []);
  return (
    <dialog ref={dialog} className="studio-dialog" onClose={onClose}>
      <div className="dialog-heading">
        <div>
          <span className="kicker">{t("studio")}</span>
          <h2>{t("settings")}</h2>
        </div>
        <button
          className="icon-button"
          aria-label={t("close")}
          onClick={() => dialog.current?.close()}
        >
          <Icon name="close" />
        </button>
      </div>
      <section>
        <h3>
          <Icon name="spark" size={18} />
          {t("assistant")}
        </h3>
        <Field label={t("provider")}>
          <Select value={provider} onChange={(e) => onProvider(e.target.value)}>
            {(providers.length
              ? providers
              : [
                  {
                    id: "ollama",
                    name: t("local"),
                    model: "",
                    configured: true,
                  },
                ]
            ).map((p) => (
              <option key={p.id} value={p.id}>
                {p.id === "platform"
                  ? c("platformDefault")
                  : p.id === "ollama"
                    ? t("local")
                    : p.name}
                {p.configured ? "" : ` · ${t("missingKey")}`}
              </option>
            ))}
          </Select>
        </Field>
        <Field label={t("model")}>
          <TextInput
            value={model}
            onChange={(e) => onModel(e.target.value)}
            placeholder={t("serverDefault")}
          />
        </Field>
        <p>{t("cloudDisclosure")}</p>
        <Button type="submit" disabled={!connected || busy} onClick={onIndex}>
          <Icon name="refresh" size={16} />
          {t("index")}
        </Button>
      </section>
    </dialog>
  );
}
