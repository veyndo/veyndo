/** Menu button following the WAI-ARIA menu pattern: arrows and Home/End move, Escape and outside presses close and return focus. */
import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { rovingIndex, useOutsidePress } from "./primitives";

export type MenuItem = {
  id: string;
  label: ReactNode;
  onSelect: () => void;
  disabled?: boolean;
  tone?: "danger";
};

export function Menu({
  label,
  items,
  trigger,
}: {
  /** Accessible name of the trigger when it has no visible text, and of the menu. */
  label: string;
  items: MenuItem[];
  /** Visible content of the trigger; defaults to the label. */
  trigger?: ReactNode;
}) {
  const id = useId();
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const button = useRef<HTMLButtonElement>(null),
    list = useRef<HTMLDivElement>(null),
    entries = useRef<(HTMLButtonElement | null)[]>([]);
  const disabled = items.map((item) => Boolean(item.disabled));
  const close = (restore = true) => {
    setOpen(false);
    if (restore) button.current?.focus();
  };
  const openAt = (index: number | null) => {
    if (index === null) return;
    setActive(index);
    setOpen(true);
  };
  useOutsidePress([button, list], open, () => close(false));
  useEffect(() => {
    if (open) entries.current[active]?.focus();
  }, [open, active]);
  return (
    <span className="vy-menu">
      <button
        ref={button}
        type="button"
        className="vy-button"
        data-tone="secondary"
        data-size="md"
        id={`${id}-trigger`}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? `${id}-menu` : undefined}
        aria-label={trigger ? label : undefined}
        onClick={() =>
          open ? close() : openAt(rovingIndex("Home", 0, disabled, "vertical"))
        }
        onKeyDown={(event) => {
          if (event.key === "ArrowDown" || event.key === "ArrowUp") {
            event.preventDefault();
            openAt(
              rovingIndex(
                event.key === "ArrowDown" ? "Home" : "End",
                0,
                disabled,
                "vertical",
              ),
            );
          }
        }}
      >
        {trigger ?? label}
      </button>
      {open && (
        <div
          ref={list}
          role="menu"
          id={`${id}-menu`}
          aria-labelledby={`${id}-trigger`}
          className="vy-menu-list"
          onKeyDown={(event) => {
            if (event.key === "Escape") {
              event.preventDefault();
              close();
              return;
            }
            if (event.key === "Tab") {
              close(false);
              return;
            }
            const next = rovingIndex(event.key, active, disabled, "vertical");
            if (next === null) return;
            event.preventDefault();
            setActive(next);
          }}
        >
          {items.map((item, index) => (
            <button
              key={item.id}
              ref={(element) => {
                entries.current[index] = element;
              }}
              type="button"
              role="menuitem"
              tabIndex={index === active ? 0 : -1}
              disabled={item.disabled}
              data-tone={item.tone}
              onClick={() => {
                close();
                item.onSelect();
              }}
            >
              {item.label}
            </button>
          ))}
        </div>
      )}
    </span>
  );
}
