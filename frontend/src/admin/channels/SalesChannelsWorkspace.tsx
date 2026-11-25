/** Discover and create channels separately from rules; shared settings and independent SaaS shops remain explicit. */
import { Badge, Button, Card, Notice } from "../../kit/ui";
import { useEffect, useState } from "react";
import type { RequestFn } from "../shell/studio-types";
import type { Category } from "../catalog/catalog-model";
import { useLocale } from "../../shared/i18n/i18n";
import { contentText } from "../../shared/i18n/content-language";
import { useChannelText } from "./channel-i18n";
import Icon from "../../shared/ui/Icon";
import { freshChannel, type Channel } from "./channel-model";
import ChannelEditor from "./ChannelEditor";
import SupplyReorder from "./SupplyReorder";
import AgenticChannels from "./AgenticChannels";
import { useAgenticChannelText } from "./agentic-channel-i18n";
import CatalogIntake from "../catalog/CatalogIntake";
import type { IntakeMode } from "../catalog/catalog-intake-model";
import MigrationWorkspace from "../../../../extensions/apps/migration/studio/MigrationWorkspace";
import { useMigrationText } from "../../../../extensions/apps/migration/studio/i18n";
import MarketplaceWorkspace from "../../../../extensions/apps/marketplaces/studio/MarketplaceWorkspace";
import "./sales-channels.css";
export default function SalesChannelsWorkspace({
  request,
  scopedRequest,
  workspace,
  onTeam,
  onDesign,
  onTest,
  sandbox = false,
}: {
  request: RequestFn;
  scopedRequest?: (target: string) => RequestFn;
  workspace: string;
  onTeam: () => void;
  onDesign?: (
    id: string,
    target?: string,
    reference?: string,
  ) => void | Promise<void>;
  sandbox?: boolean;
  onTest?: (target: string) => void | Promise<void>;
}) {
  const migrationText = useMigrationText();
  const [migration, setMigration] = useState(false);
  const ai = useAgenticChannelText();
  const [agenticChannel, setAgenticChannel] = useState("default");
  const [agentic, setAgentic] = useState(false),
    [intake, setIntake] = useState<IntakeMode>();
  const t = useChannelText(),
    { locale } = useLocale();
  const [channels, setChannels] = useState<Channel[]>([]),
    [selected, setSelected] = useState<Channel | null>(null),
    [settings, setSettings] = useState<{
      locales: string[];
      mainLocale: string;
      countries: string[];
    }>(),
    [categories, setCategories] = useState<Category[]>([]),
    [manage, setManage] = useState(false),
    [rights, setRights] = useState<string[]>([]),
    [marketplace, setMarketplace] = useState(false),
    [error, setError] = useState("");
  useEffect(() => {
    let active = true;
    setSelected(null);
    setAgentic(false);
    setAgenticChannel("default");
    setIntake(undefined);
    setMarketplace(false);
    setMigration(false);
    setManage(false);
    setError("");
    Promise.all([
      request("/api/automation"),
      request("/api/merchant/commerce"),
      request("/api/merchant/categories"),
      request("/api/auth/access"),
    ])
      .then(([a, c, k, p]) => {
        if (active) {
          setChannels(a.channels);
          setRights(p.permissions);
          setSettings(c.data);
          setCategories(k.elements);
          setManage(
            p.permissions.includes("channels.manage") ||
              p.permissions.includes("settings.write"),
          );
        }
      })
      .catch((e) => {
        if (active) setError(e.message);
      });
    return () => {
      active = false;
    };
  }, [request, workspace]);
  const save = (channel: Channel) => {
    setChannels((old) => [...old.filter((c) => c.id !== channel.id), channel]);
    setSelected(channel);
  };
  if (migration)
    return (
      <MigrationWorkspace
        onTest={onTest}
        key={workspace}
        request={request}
        scopedRequest={scopedRequest}
        workspace={workspace}
        rights={rights}
        mainLocale={settings?.mainLocale ?? locale}
        onBack={() => setMigration(false)}
        onDesign={
          onDesign
            ? (target, reference) => onDesign("default", target, reference)
            : undefined
        }
      />
    );
  if (agentic)
    return (
      <div className="studio-page">
        <AgenticChannels
          selectedChannel={agenticChannel}
          onChannel={setAgenticChannel}
          request={request}
          canRead={rights.includes("catalog.read")}
          canImport={rights.includes("catalog.write")}
          onImport={setIntake}
          onBack={() => setAgentic(false)}
          channels={channels}
          mainLocale={settings?.mainLocale ?? locale}
        />
        <SupplyReorder request={request} rights={rights} />
        {intake && (
          <CatalogIntake
            initialMode={intake}
            onClose={() => setIntake(undefined)}
          />
        )}
      </div>
    );
  if (marketplace && selected && settings)
    return (
      <MarketplaceWorkspace
        request={request}
        channel={selected.id}
        name={contentText(selected.data.name, locale, settings.mainLocale)}
        mainLocale={settings.mainLocale}
        rights={rights}
        onBack={() => setMarketplace(false)}
      />
    );
  return (
    <div className="studio-page sales-channel-workspace">
      <div className="page-intro">
        <h1>{t("title")}</h1>
        <p>{t("hint")}</p>
        {!sandbox && !selected && rights.includes("apps.read") && (
          <Button type="submit" onClick={() => setMigration(true)}>
            <Icon name="layers" />
            {migrationText("title")}
          </Button>
        )}
        {!selected && (
          <Button type="button" onClick={() => setAgentic(true)}>
            <Icon name="spark" />
            {ai("title")}
          </Button>
        )}
        {!selected && (
          <Button
            tone="primary"
            type="submit"
            disabled={!settings || !manage}
            onClick={() => setSelected(freshChannel(settings!.mainLocale))}
          >
            + {t("new")}
          </Button>
        )}
      </div>
      {selected && settings ? (
        <ChannelEditor
          key={selected.id}
          initial={selected}
          request={request}
          languages={settings.locales}
          mainLocale={settings.mainLocale}
          categories={categories}
          countries={settings.countries ?? []}
          workspace={workspace}
          canWrite={manage}
          onBack={() => setSelected(null)}
          onSaved={save}
          onMarketplace={() => setMarketplace(true)}
          onDesign={onDesign ? () => onDesign(selected.id) : undefined}
          onDeleted={() => {
            setChannels((old) => old.filter((c) => c.id !== selected.id));
            setSelected(null);
            setMarketplace(false);
          }}
        />
      ) : (
        <>
          <div className="channel-card-grid">
            {channels.map((c) => (
              <Card as="article" className="channel-card" key={c.id}>
                <span className="channel-avatar">
                  <Icon
                    name={
                      c.data.kind === "marketplace"
                        ? "layers"
                        : c.data.kind === "pos"
                          ? "register"
                          : c.data.kind === "headless"
                            ? "code"
                            : "globe"
                    }
                    size={28}
                  />
                </span>
                <Badge>
                  {t(
                    !c.data.active
                      ? "inactive"
                      : c.data.visibility === "private"
                        ? "private"
                        : "active",
                  )}
                </Badge>
                {c.id === "default" && <Badge>{t("default")}</Badge>}
                <h2>
                  {contentText(
                    c.data.name,
                    locale,
                    settings?.mainLocale ?? "en-GB",
                  ) || c.id}
                </h2>
                <p>
                  {t(c.data.kind)} · {c.data.locales.length} ·{" "}
                  {c.data.productIds.length
                    ? `${t("selected")}: ${c.data.productIds.length}`
                    : t("all")}
                </p>
                <Button type="submit" onClick={() => setSelected(c)}>
                  {t("edit")} ↗
                </Button>
              </Card>
            ))}
          </div>
          <Card as="aside">
            <p>{t("independent")}</p>
            <Button type="submit" onClick={onTeam}>
              {t("team")} ↗
            </Button>
          </Card>
        </>
      )}
      {error && <Notice tone="danger">{error}</Notice>}
    </div>
  );
}
