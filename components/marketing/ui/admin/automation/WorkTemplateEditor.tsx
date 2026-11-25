/** Bounded field-to-event mapping over the original native operations, never a second executor. */
import {
  Card,
  Field,
  Notice,
  Select,
  TextInput,
} from "../../../../../frontend/src/kit/ui";
import { useState } from "react";
import {
  scalarFields,
  type WorkTemplate,
  type WorkStep,
} from "./work-template";
import type { RequestFn } from "../../../../../frontend/src/admin/shell/studio-types";
import { useRead } from "../../../../../frontend/src/admin/shell/useRead";
import { useWorkTemplateText } from "./work-template-i18n";
export default function WorkTemplateEditor({
  value,
  onChange,
  request,
}: {
  request?: RequestFn;
  value?: WorkTemplate;
  onChange: (v: WorkTemplate) => void;
}) {
  const t = useWorkTemplateText();
  const [selected, setSelected] = useState<Record<number, string>>({});
  const taskId = value?.taskId;
  const run = useRead(
    request,
    taskId ? `/api/agent/runs/${encodeURIComponent(taskId)}` : null,
  );
  if (!value) return <p>{t("empty")}</p>;
  const runSteps: WorkStep[] | undefined =
    run.data?.state === "ready" &&
    run.data.approvalRequired === false &&
    run.data.execution?.total > 0 &&
    run.data.execution.completed === run.data.execution.total
      ? run.data.preview?.proposal?.operations
      : undefined;
  const current =
    run.status === "success" || run.status === "error"
      ? {
          steps: runSteps,
          error:
            run.status === "error" ||
            !Array.isArray(runSteps) ||
            !runSteps.length,
        }
      : undefined;
  if (!request || current?.error)
    return <Notice tone="danger">{t("unavailable")}</Notice>;
  if (!current?.steps) return <p role="status">{t("loading")}</p>;
  const steps = current.steps;
  return (
    <section className="work-template-editor">
      <h3>{t("title")}</h3>
      <p>{t("hint")}</p>
      <Field label={t("limit")}>
        <TextInput
          type="number"
          min={1}
          max={100}
          required
          value={value.dailyLimit}
          onChange={(e) =>
            onChange({ ...value, dailyLimit: Number(e.target.value) })
          }
        />
      </Field>
      {steps.map((step, index) => {
        let args: unknown;
        try {
          args = JSON.parse(step.arguments_json);
        } catch {
          args = {};
        }
        const fields = scalarFields(args);
        const pointer = selected[index] ?? fields[0]?.pointer ?? "";
        const binding = value.bindings.find(
          (b) => b.step === index && b.pointer === pointer,
        );
        const source = binding?.source ?? "";
        return (
          <Card as="article" key={index}>
            <strong>
              {index + 1}. {step.label}
            </strong>
            <p>
              <code>{step.name}</code>
            </p>
            <div className="connector-settings">
              <Field label={t("field")}>
                <Select
                  value={pointer}
                  onChange={(e) =>
                    setSelected({ ...selected, [index]: e.target.value })
                  }
                >
                  {fields.map((f) => (
                    <option key={f.pointer} value={f.pointer}>
                      {f.pointer}
                    </option>
                  ))}
                </Select>
              </Field>
              <Field label={t("source")}>
                <Select
                  value={source}
                  onChange={(e) =>
                    onChange({
                      ...value,
                      bindings: [
                        ...value.bindings.filter(
                          (b) => b.step !== index || b.pointer !== pointer,
                        ),
                        ...(e.target.value
                          ? [{ step: index, pointer, source: e.target.value }]
                          : []),
                      ],
                    })
                  }
                >
                  <option value="">{t("fixed")}</option>
                  {typeof fields.find((f) => f.pointer === pointer)?.value ===
                    "string" && (
                    <>
                      <option value="event.productId">{t("product")}</option>

                      <option value="event.orderId">{t("order")}</option>
                      <option value="event.documentId">{t("document")}</option>
                      {steps.map((_, i) => (
                        <option key={i} value={`run.key:step_${i}`}>
                          {t("generated")} · {i + 1}
                        </option>
                      ))}
                    </>
                  )}
                  {typeof fields.find((f) => f.pointer === pointer)?.value ===
                    "number" && (
                    <option value="event.revision">{t("revision")}</option>
                  )}
                </Select>
              </Field>
            </div>
            <p>
              <code>
                {String(fields.find((f) => f.pointer === pointer)?.value ?? "")}
              </code>
            </p>
            <details>
              <summary>{t("exact")}</summary>
              <pre>{JSON.stringify(args, null, 2)}</pre>
            </details>
          </Card>
        );
      })}
      <p className="muted">{t("revisions")}</p>
    </section>
  );
}
