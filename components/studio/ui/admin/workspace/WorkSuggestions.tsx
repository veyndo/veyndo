/** Actual permission-filtered work links open native owners; late responses cannot survive a scope change. */
import { useRead } from "../../../../../frontend/src/admin/shell/useRead";
import { Button, Card, Notice } from "../../../../../frontend/src/kit/ui";
import { useState } from "react";
import { useStudio } from "../../../../../frontend/src/admin/shell/StudioContext";
import { useAgentWorkspaceText } from "../../../../../frontend/src/shared/i18n/agent-workspace-i18n";
import Icon from "../../../../../frontend/src/shared/ui/Icon";
type Item = {
  id: string;
  kind: "stock" | "run" | "translation" | "app";
  entity?: string;
  runId?: string;
  translationJob?: string;
  app?: string;
};
export default function WorkSuggestions() {
  const { request, openEntity, selectTab, load, intent } = useStudio(),
    t = useAgentWorkspaceText();
  const [failed, setError] = useState(false);
  const work = useRead(request, "/api/studio/work");
  const items: Item[] = Array.isArray(work.data?.items) ? work.data.items : [];
  const error = failed || work.status === "error";
  const open = async (item: Item) => {
    if (item.kind === "stock" && item.entity)
      openEntity("productData", item.entity);
    else if (item.kind === "translation") {
      const url = new URL(location.href);
      url.searchParams.set("settings", "languages");
      url.searchParams.set("translationJob", item.translationJob ?? "");
      history.replaceState(null, "", url);
      selectTab("commerce");
    } else if (item.kind === "run") {
      try {
        const run = await request(`/api/agent/runs/${item.runId}`);
        if (run.conversationId) await load(run.conversationId);
        selectTab("assistant");
      } catch {
        setError(true);
      }
    } else intent(`${t("app")}: ${item.app}`);
  };
  return (
    <Card className="personal-goals">
      <h2>
        <Icon name="spark" />
        {t("work")}
      </h2>
      <p>{t("hint")}</p>
      <Button
        type="submit"
        onClick={() => {
          setError(false);
          void work.refetch();
        }}
      >
        {t("reload")}
      </Button>
      {error ? (
        <Notice tone="danger">{t("error")}</Notice>
      ) : !items.length ? (
        <p>{t("empty")}</p>
      ) : (
        <div className="personal-cards">
          {items.map((item) => (
            <Button type="button" key={item.id} onClick={() => void open(item)}>
              <Icon name={item.kind === "stock" ? "box" : "pulse"} />
              {t(item.kind)}
              {item.entity && <small>{item.entity}</small>}
            </Button>
          ))}
        </div>
      )}
    </Card>
  );
}
