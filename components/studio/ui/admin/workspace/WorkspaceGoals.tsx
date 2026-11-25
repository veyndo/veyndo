/** Goals reference the existing proposal owner; no parallel worker, executor or approval queue. */
import { useRead } from "../../../../../frontend/src/admin/shell/useRead";
import {
  Button,
  Card,
  Field,
  Notice,
  Select,
  TextInput,
} from "../../../../../frontend/src/kit/ui";
import { useState } from "react";
import { canOpenStudio } from "../../../../../frontend/src/admin/shell/permissions";
import type { Tab } from "../../../../../frontend/src/admin/shell/navigation";
import type { RequestFn } from "../../../../../frontend/src/admin/shell/studio-types";
import type { Goal } from "./workspace-model";
import { useWorkspaceText } from "./workspace-i18n";
import Icon from "../../../../../frontend/src/shared/ui/Icon";
import { prepareGoalFlow } from "../../../../marketing/ui/admin/automation/goal-template";
import WorkspaceTask, {
  type WorkspaceTaskValue as Task,
} from "./WorkspaceTask";
export default function WorkspaceGoals({
  goals,
  onChange,
  onAgent,
  request,
  rights,
  onOpen,
}: {
  goals: Goal[];
  onChange: (goals: Goal[]) => void;
  onAgent: (title: string) => void;
  request: RequestFn;
  rights: string[];
  onOpen: (tab: Tab) => void;
}) {
  const t = useWorkspaceText(),
    [title, setTitle] = useState("");
  const allowed = canOpenStudio("assistant", rights);
  const taskRead = useRead(request, allowed ? "/api/agent/tasks" : null);
  const tasks: Task[] = taskRead.data?.tasks ?? [];
  const error = taskRead.status === "error" ? t("error") : "";
  const patch = (id: string, value: Partial<Goal>) =>
    onChange(goals.map((g) => (g.id === id ? { ...g, ...value } : g)));
  return (
    <Card className="personal-goals">
      <h2>
        <Icon name="spark" />
        {t("goals")}
      </h2>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (!title.trim() || goals.length >= 30) return;
          onChange([
            ...goals,
            {
              id: `goal_${crypto.randomUUID().replaceAll("-", "").slice(0, 16)}`,
              title: title.trim(),
              status: "planned",
              taskId: null,
            },
          ]);
          setTitle("");
        }}
      >
        <Field label={t("goal")}>
          <TextInput
            value={title}
            maxLength={1000}
            onChange={(e) => setTitle(e.target.value)}
            placeholder={t("placeholder")}
          />
        </Field>
        <Button type="submit" disabled={!title.trim() || goals.length >= 30}>
          <Icon name="plus" />
          {t("addGoal")}
        </Button>
      </form>
      <p>{t("agentHint")}</p>
      {goals.map((g) => (
        <article className="personal-goal" key={g.id}>
          <strong>{g.title}</strong>
          <Field label={t("status")}>
            <Select
              value={g.status}
              onChange={(e) =>
                patch(g.id, { status: e.target.value as Goal["status"] })
              }
            >
              {(
                ["planned", "working", "review", "done", "stopped"] as const
              ).map((s) => (
                <option value={s} key={s}>
                  {t(s)}
                </option>
              ))}
            </Select>
          </Field>
          {allowed && (
            <select
              aria-label={t("tasks")}
              value={g.taskId ?? ""}
              onChange={(e) => patch(g.id, { taskId: e.target.value || null })}
            >
              <option value="">{t("tasks")}</option>
              {tasks.map((task) => (
                <option key={task.id} value={task.id}>
                  {task.preview.proposal?.summary}
                </option>
              ))}
            </select>
          )}
          {rights.includes("automation.manage") && (
            <button
              onClick={() => {
                prepareGoalFlow(g.title);
                onOpen("automation");
              }}
            >
              {t("flow")} ↗
            </button>
          )}
          <button disabled={!allowed} onClick={() => onAgent(g.title)}>
            {t("plan")} ↗
          </button>
          <button
            aria-label={`${t("remove")}: ${g.title}`}
            onClick={() => onChange(goals.filter((a) => a.id !== g.id))}
          >
            ×
          </button>
        </article>
      ))}
      {error && <Notice tone="danger">{error}</Notice>}
      {allowed && (
        <details>
          <summary>
            {t("tasks")} · {tasks.length}
          </summary>
          {tasks.map((task) => (
            <WorkspaceTask
              key={task.id}
              task={task}
              request={request}
              onFlow={
                rights.includes("automation.manage")
                  ? () => onOpen("automation")
                  : undefined
              }
              onTranslations={(jobId) => {
                const target = new URL(location.href);
                target.searchParams.set("settings", "languages");
                target.searchParams.set("translationJob", jobId);
                history.replaceState(null, "", target);
                onOpen("commerce");
              }}
            />
          ))}
        </details>
      )}
      {canOpenStudio("automation", rights) && (
        <details>
          <summary>{t("flow")}</summary>
          <p>{t("flowHint")}</p>
          <button onClick={() => onOpen("automation")}>{t("flow")} ↗</button>
        </details>
      )}
    </Card>
  );
}
