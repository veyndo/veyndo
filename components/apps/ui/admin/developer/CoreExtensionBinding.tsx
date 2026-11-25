/** One understandable native-object selection; relationships, uniqueness and scopes compile together. */
import { Field, Select } from "../../../../../frontend/src/kit/ui";
import type { Entity } from "../../../../../frontend/src/shared/apps/native/types";
import { useCoreExtensionText } from "../../../../../frontend/src/shared/i18n/core-extension-i18n";
import { bindExtension, type CoreObject } from "./core-extension-model";
export default function CoreExtensionBinding({
  entity,
  onChange,
}: {
  entity: Entity;
  onChange: (e: Entity) => void;
}) {
  const t = useCoreExtensionText();
  return (
    <div className="app-data-safety">
      <Field label={t("binding")}>
        <Select
          value={entity.extends?.object ?? ""}
          disabled={!entity.extends && entity.fields.length >= 16}
          onChange={(e) =>
            onChange(
              bindExtension(
                entity,
                (e.target.value || undefined) as CoreObject | undefined,
              ),
            )
          }
        >
          <option value="">{t("independent")}</option>
          {(["product", "customer", "order", "company"] as const).map(
            (object) => (
              <option value={object} key={object}>
                {t(object)}
              </option>
            ),
          )}
        </Select>
      </Field>
      <p>{t("hint")}</p>
    </div>
  );
}
