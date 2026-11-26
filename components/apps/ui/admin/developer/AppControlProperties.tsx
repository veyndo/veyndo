/** DataField and one-level container references are edited against the current typed model rather than free text. */
import { Button, Field, Select } from "../../../../../frontend/src/kit/ui";
import { useState } from "react";
import type {
  Block,
  Manifest,
  NativeView,
} from "../../../../../frontend/src/shared/apps/native/types";
import { isDataBlock } from "../../../../../frontend/src/shared/apps/native/types";
import { useAppStudioText } from "../../../../../frontend/src/shared/i18n/app-studio-i18n";
import { contentText } from "../../../../../frontend/src/shared/i18n/content-language";
import { useContentLanguage } from "../../../../../frontend/src/shared/i18n/ContentLanguage";
import { accepts, inheritedContext } from "./control-model";
import { binding } from "./app-model";
import AppLogicEditor from "./AppLogicEditor";
export default function AppControlProperties({
  block,
  manifest,
  view,
  onChange,
}: {
  block: Block;
  manifest: Manifest;
  view: NativeView;
  onChange: (b: Block) => void;
}) {
  const { a, locale } = useAppStudioText(),
    { mainLocale } = useContentLanguage();
  const [code, setCode] = useState(false);
  const entity = manifest.entities.find((e) => e.name === block.entity);
  return (
    <>
      {block.kind === "table" && (
        <label className="app-check">
          <input
            type="checkbox"
            checked={block.inlineEdit ?? false}
            onChange={(e) =>
              onChange({
                ...block,
                inlineEdit: e.target.checked,
                writeAction: e.target.checked
                  ? `save_${block.entity?.slice(0, 27)}`
                  : undefined,
              })
            }
          />
          {a("inlineEdit")}
        </label>
      )}
      {isDataBlock(block.kind) && (
        <Field label={a("entity")}>
          <Select
            value={block.entity ?? ""}
            onChange={(e) => {
              const model = manifest.entities.find(
                (n) => n.name === e.target.value,
              );
              onChange({
                ...block,
                ...binding(block.kind, e.target.value),
                dataField: model?.fields.find(
                  (f) => !f.coreReference && accepts(block.kind, f),
                )?.name,
                contextBinding: inheritedContext(view, e.target.value),
              });
            }}
          >
            {manifest.entities.map((e) => (
              <option value={e.name} key={e.name}>
                {contentText(e.label, locale, mainLocale) || e.name}
              </option>
            ))}
          </Select>
        </Field>
      )}
      {entity && !["form", "table", "cards"].includes(block.kind) && (
        <Field label={a("sourceField")}>
          <Select
            value={block.dataField ?? ""}
            onChange={(e) => onChange({ ...block, dataField: e.target.value })}
          >
            <option value="">—</option>
            {entity.fields
              .filter((f) => accepts(block.kind, f))
              .map((f) => (
                <option value={f.name} key={f.name}>
                  {contentText(f.label, locale, mainLocale) || f.name}
                </option>
              ))}
          </Select>
        </Field>
      )}
      {["frame", "tabs"].includes(block.kind) && (
        <Field label={a("children")}>
          <Select
            multiple
            value={block.childBlocks ?? []}
            onChange={(e) =>
              onChange({
                ...block,
                childBlocks: Array.from(e.target.selectedOptions).map(
                  (o) => o.value,
                ),
              })
            }
          >
            {view.blocks
              .filter(
                (b) =>
                  b.id !== block.id &&
                  !b.childBlocks?.length &&
                  !view.blocks.some(
                    (parent) =>
                      parent.id !== block.id &&
                      parent.childBlocks?.includes(b.id),
                  ),
              )
              .map((b) => (
                <option key={b.id} value={b.id}>
                  {contentText(b.title, locale, mainLocale) || b.id}
                </option>
              ))}
          </Select>
        </Field>
      )}
      {["button", "textbox", "combobox", "checkbox", "datepicker"].includes(
        block.kind,
      ) && (
        <Button type="button" onClick={() => setCode(true)}>
          {a("eventsCode")}
        </Button>
      )}
      {code && (
        <AppLogicEditor
          manifest={manifest}
          view={view}
          block={block}
          onChange={onChange}
          onClose={() => setCode(false)}
        />
      )}
    </>
  );
}
