/** Builder form of a part's configuration, generated from its JSON Schema; hosts supply labels and special pickers. */
import type { ReactNode } from "react";
import { Checkbox, Field, Select, TextInput } from "../ui";
import type { AnyPart } from "./contract";

type Schema = {
  type?: string | string[];
  enum?: unknown[];
  format?: string;
  minimum?: number;
  maximum?: number;
  default?: unknown;
  properties?: Record<string, Schema>;
  required?: string[];
};

export type PartFormProps = {
  part: AnyPart;
  value: Record<string, unknown>;
  onChange: (value: Record<string, unknown>) => void;
  /** The merchant-facing label of a configuration field (localized by the host). */
  label: (field: string) => string;
  /** The label of one enum choice. */
  choice?: (field: string, value: string) => string;
  /** Renders fields with a special format, for example a product picker. */
  renderFormat?: (
    format: string,
    field: string,
    value: unknown,
    set: (value: unknown) => void,
  ) => ReactNode;
  disabled?: boolean;
};

export function PartForm({
  part,
  value,
  onChange,
  label,
  choice = (_, v) => v,
  renderFormat,
  disabled,
}: PartFormProps) {
  const schema = part.configSchema() as Schema;
  const required = new Set(schema.required ?? []);
  return (
    <div className="vy-sf-form">
      {Object.entries(schema.properties ?? {}).map(([field, spec]) => {
        const current = value[field] ?? spec.default;
        const set = (next: unknown) => onChange({ ...value, [field]: next });
        const type = Array.isArray(spec.type) ? spec.type[0] : spec.type;
        if (spec.format && renderFormat)
          return (
            <div key={field}>
              {renderFormat(spec.format, field, current, set)}
            </div>
          );
        if (type === "boolean")
          return (
            <Checkbox
              key={field}
              label={label(field)}
              checked={current === true}
              disabled={disabled}
              onChange={(event) => set(event.target.checked)}
            />
          );
        if (spec.enum)
          return (
            <Field
              key={field}
              label={label(field)}
              required={required.has(field)}
            >
              <Select
                value={String(current ?? "")}
                disabled={disabled}
                onChange={(event) => set(event.target.value)}
              >
                {spec.enum.map((option) => (
                  <option key={String(option)} value={String(option)}>
                    {choice(field, String(option))}
                  </option>
                ))}
              </Select>
            </Field>
          );
        const numeric = type === "number" || type === "integer";
        return (
          <Field
            key={field}
            label={label(field)}
            required={required.has(field)}
          >
            <TextInput
              type={numeric ? "number" : "text"}
              min={spec.minimum}
              max={spec.maximum}
              value={current === undefined ? "" : String(current)}
              disabled={disabled}
              onChange={(event) =>
                set(
                  numeric
                    ? event.target.value === ""
                      ? undefined
                      : Number(event.target.value)
                    : event.target.value,
                )
              }
            />
          </Field>
        );
      })}
    </div>
  );
}
