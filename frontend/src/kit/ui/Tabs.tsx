/** Tabs following the WAI-ARIA tabs pattern: roving tabindex, arrow/Home/End keys, automatic activation, linked panels. */
import { useId, useRef, type ReactNode } from "react";
import { rovingIndex, useControllable, type Orientation } from "./primitives";

export type TabItem = {
  id: string;
  label: ReactNode;
  panel: ReactNode;
  disabled?: boolean;
};

export function Tabs({
  label,
  items,
  value,
  defaultValue,
  onChange,
  orientation = "horizontal",
}: {
  /** Accessible name of the tab list. */
  label: string;
  items: TabItem[];
  value?: string;
  defaultValue?: string;
  onChange?: (id: string) => void;
  orientation?: Orientation;
}) {
  const base = useId();
  const [selected, select] = useControllable(
    value,
    defaultValue ?? items.find((item) => !item.disabled)?.id ?? "",
    onChange,
  );
  const tabs = useRef<(HTMLButtonElement | null)[]>([]);
  const current = Math.max(
    0,
    items.findIndex((item) => item.id === selected),
  );
  return (
    <div className="vy-tabs" data-orientation={orientation}>
      <div role="tablist" aria-label={label} aria-orientation={orientation}>
        {items.map((item, index) => (
          <button
            key={item.id}
            ref={(element) => {
              tabs.current[index] = element;
            }}
            type="button"
            role="tab"
            id={`${base}-tab-${item.id}`}
            aria-selected={index === current}
            aria-controls={`${base}-panel-${item.id}`}
            tabIndex={index === current ? 0 : -1}
            disabled={item.disabled}
            onClick={() => select(item.id)}
            onKeyDown={(event) => {
              const next = rovingIndex(
                event.key,
                index,
                items.map((entry) => Boolean(entry.disabled)),
                orientation,
              );
              if (next === null) return;
              event.preventDefault();
              select(items[next].id);
              tabs.current[next]?.focus();
            }}
          >
            {item.label}
          </button>
        ))}
      </div>
      {items.map((item, index) => (
        <div
          key={item.id}
          role="tabpanel"
          id={`${base}-panel-${item.id}`}
          aria-labelledby={`${base}-tab-${item.id}`}
          hidden={index !== current}
          tabIndex={0}
        >
          {index === current && item.panel}
        </div>
      ))}
    </div>
  );
}
