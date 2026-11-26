/** B2B controls edit the shared revisioned settings aggregate; no separate policy store or checkout engine. */
import { Button, Card, Notice } from "../../../../../frontend/src/kit/ui";
import "../../../../../frontend/src/shared/b2b/b2b.css";
import "./b2b-settings.css";
import { b2bChanged } from "../../../../../frontend/src/admin/shell/useB2BNavigation";
import { useEffect } from "react";
import type { RequestFn } from "../../../../../frontend/src/admin/shell/studio-types";
import type { Config } from "../../../../../frontend/src/shared/api/shop-api";
import { useSettingsDraft } from "./useSettingsDraft";
import SettingsSaveBar from "./SettingsSaveBar";
import EntityHistory from "../../../../../frontend/src/shared/history/EntityHistory";
import {
  useB2BText,
  type B2BWord,
} from "../../../../../frontend/src/shared/i18n/b2b-i18n";
import Icon, {
  type IconName,
} from "../../../../../frontend/src/shared/ui/Icon";
export type B2BFeatures = {
  enabled: boolean;
  quotes: boolean;
  approvals: boolean;
  lists: boolean;
  quickOrder: boolean;
  edi: boolean;
  catalogs: boolean;
};
export const defaultB2BFeatures: B2BFeatures = {
  enabled: true,
  quotes: true,
  approvals: true,
  lists: true,
  quickOrder: true,
  edi: true,
  catalogs: true,
};
export default function B2BSettings({
  request,
  canWrite,
  onDirty,
  onCompanies,
}: {
  request: RequestFn;
  canWrite: boolean;
  onDirty?: (v: boolean) => void;
  onCompanies?: () => void;
}) {
  const b = useB2BText(),
    state = useSettingsDraft<Config & { b2b?: B2BFeatures }>(
      request,
      "/api/merchant/commerce",
    );
  useEffect(() => onDirty?.(state.dirty), [state.dirty, onDirty]);
  useEffect(() => {
    if (state.saved) window.dispatchEvent(new Event(b2bChanged));
  }, [state.saved]);
  if (!state.value)
    return (
      <p role={state.error ? "alert" : "status"}>
        {state.error || b("loading")}
      </p>
    );
  const features = { ...defaultB2BFeatures, ...state.value.data.b2b };
  const icons: Record<keyof Omit<B2BFeatures, "enabled">, IconName> = {
    quotes: "chat",
    approvals: "check",
    lists: "layers",
    quickOrder: "box",
    edi: "code",
    catalogs: "lock",
  };
  const featureWords = {
    quotes: ["featureQuotes", "featureQuotesHint"],
    approvals: ["featureApprovals", "featureApprovalsHint"],
    lists: ["featureLists", "featureListsHint"],
    quickOrder: ["featureQuickOrder", "featureQuickOrderHint"],
    edi: ["featureEdi", "featureEdiHint"],
    catalogs: ["featureCatalogs", "featureCatalogsHint"],
  } as const satisfies Record<keyof typeof icons, readonly [B2BWord, B2BWord]>;
  const change = (patch: Partial<B2BFeatures>) =>
    state.change({ ...state.value!.data, b2b: { ...features, ...patch } });
  return (
    <Card className="settings-panel b2b-settings">
      <header className="settings-panel-header">
        <div>
          <h2>{b("purchasingSettings")}</h2>
          <p>{b("purchasingSettingsHint")}</p>
        </div>
        {features.enabled && onCompanies && (
          <Button type="submit" onClick={onCompanies}>
            {b("purchasingCompanies")} ↗
          </Button>
        )}
      </header>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (canWrite) void state.save();
        }}
      >
        <fieldset className="intl-fields" disabled={!canWrite || state.busy}>
          <label className="checkbox-label">
            <input
              type="checkbox"
              checked={features.enabled}
              onChange={(e) => change({ enabled: e.target.checked })}
            />
            {b("purchasingEnabled")}
          </label>
          <div className="b2b-feature-grid">
            {(Object.keys(icons) as (keyof typeof icons)[]).map((key) => (
              <label className="b2b-feature-card" key={key}>
                <Icon name={icons[key]} />
                <span>
                  <strong>{b(featureWords[key][0])}</strong>
                  <small>{b(featureWords[key][1])}</small>
                </span>
                <input
                  type="checkbox"
                  checked={features[key]}
                  onChange={(e) => change({ [key]: e.target.checked })}
                />
              </label>
            ))}
          </div>
          {state.error && <Notice tone="danger">{state.error}</Notice>}
          <p>{b("purchasingBudgetsHint")}</p>
          <SettingsSaveBar
            dirty={state.dirty}
            busy={state.busy}
            saved={state.saved}
            canWrite={canWrite}
          />
        </fieldset>
      </form>
      <EntityHistory
        request={request}
        entity="settings"
        id="default"
        revision={state.value.revision}
        dirty={state.dirty || state.busy}
        onRestored={async () => {
          await state.reload();
          window.dispatchEvent(new Event(b2bChanged));
        }}
      />
    </Card>
  );
}
