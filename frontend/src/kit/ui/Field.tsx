/** Form controls: a Field binds label, hint and error to its control by id; controls read that binding from context. */
import {
  createContext,
  forwardRef,
  useContext,
  useId,
  type InputHTMLAttributes,
  type ReactNode,
  type SelectHTMLAttributes,
  type TextareaHTMLAttributes,
} from "react";
import { useControllable } from "./primitives";

type Binding = { id: string; describedBy?: string; invalid: boolean };
const FieldContext = createContext<Binding | null>(null);

/** The id, description and validity a control inside a Field must carry. */
export function useField(id?: string): Binding {
  const own = useId();
  return useContext(FieldContext) ?? { id: id ?? own, invalid: false };
}

export function Field({
  label,
  hint,
  error,
  required = false,
  children,
}: {
  label: ReactNode;
  hint?: ReactNode;
  /** Shown instead of the hint; marks the control invalid. */
  error?: ReactNode;
  required?: boolean;
  children: ReactNode;
}) {
  const id = useId();
  const note = error ? `${id}-error` : hint ? `${id}-hint` : undefined;
  return (
    <div className="vy-field" data-invalid={error ? true : undefined}>
      <label htmlFor={id}>
        {label}
        {required && <span aria-hidden="true"> *</span>}
      </label>
      <FieldContext.Provider
        value={{ id, describedBy: note, invalid: Boolean(error) }}
      >
        {children}
      </FieldContext.Provider>
      {error ? (
        <p id={note} className="vy-field-error">
          {error}
        </p>
      ) : (
        hint && (
          <p id={note} className="vy-field-hint">
            {hint}
          </p>
        )
      )}
    </div>
  );
}

const bind = (field: Binding, describedBy?: string) => ({
  id: field.id,
  "aria-invalid": field.invalid || undefined,
  "aria-describedby":
    [field.describedBy, describedBy].filter(Boolean).join(" ") || undefined,
});

export const TextInput = forwardRef<
  HTMLInputElement,
  InputHTMLAttributes<HTMLInputElement>
>(function TextInput({ className, id, ...rest }, ref) {
  const field = useField(id);
  return (
    <input
      ref={ref}
      className={["vy-input", className].filter(Boolean).join(" ")}
      {...rest}
      {...bind(field, rest["aria-describedby"])}
    />
  );
});

export const TextArea = forwardRef<
  HTMLTextAreaElement,
  TextareaHTMLAttributes<HTMLTextAreaElement>
>(function TextArea({ className, id, ...rest }, ref) {
  const field = useField(id);
  return (
    <textarea
      ref={ref}
      className={["vy-input", className].filter(Boolean).join(" ")}
      {...rest}
      {...bind(field, rest["aria-describedby"])}
    />
  );
});

export const Select = forwardRef<
  HTMLSelectElement,
  SelectHTMLAttributes<HTMLSelectElement>
>(function Select({ className, id, ...rest }, ref) {
  const field = useField(id);
  return (
    <select
      ref={ref}
      className={["vy-input", "vy-select", className].filter(Boolean).join(" ")}
      {...rest}
      {...bind(field, rest["aria-describedby"])}
    />
  );
});

/** A checkbox with its own label; usable inside or outside a Field. */
export const Checkbox = forwardRef<
  HTMLInputElement,
  Omit<InputHTMLAttributes<HTMLInputElement>, "type"> & { label: ReactNode }
>(function Checkbox({ label, className, ...rest }, ref) {
  return (
    <label className={["vy-check", className].filter(Boolean).join(" ")}>
      <input ref={ref} type="checkbox" {...rest} />
      <span>{label}</span>
    </label>
  );
});

/** An on/off switch (`role="switch"`); Space and Enter toggle it like a button. */
export function Switch({
  label,
  checked,
  defaultChecked = false,
  onChange,
  disabled,
}: {
  label: ReactNode;
  checked?: boolean;
  defaultChecked?: boolean;
  onChange?: (checked: boolean) => void;
  disabled?: boolean;
}) {
  const [on, set] = useControllable(checked, defaultChecked, onChange);
  const id = useId();
  return (
    <span className="vy-switch">
      <button
        type="button"
        role="switch"
        id={id}
        aria-checked={on}
        disabled={disabled}
        onClick={() => set(!on)}
      >
        <span aria-hidden="true" />
      </button>
      <label htmlFor={id}>{label}</label>
    </span>
  );
}
