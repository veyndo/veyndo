/** Headless primitives: focus containment, roving focus and controlled-or-uncontrolled state, without markup or text. */
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type RefObject,
} from "react";

export const focusableSelector =
  'button:not(:disabled),input:not(:disabled),textarea:not(:disabled),select:not(:disabled),a[href],[tabindex]:not([tabindex="-1"])';

/** The elements inside `root` that take focus by Tab, in document order. */
export function focusables(root: HTMLElement | null): HTMLElement[] {
  return root
    ? [...root.querySelectorAll<HTMLElement>(focusableSelector)].filter(
        (element) => !element.closest("[inert]"),
      )
    : [];
}

/**
 * Keeps Tab inside `root` while `active`, calls `onEscape` for Escape, focuses the
 * first focusable element (or `root`) on activation and restores the previous focus.
 */
export function useFocusTrap(
  root: RefObject<HTMLElement | null>,
  active: boolean,
  onEscape?: () => void,
) {
  const escape = useRef(onEscape);
  escape.current = onEscape;
  useEffect(() => {
    if (!active) return;
    const previous = document.activeElement as HTMLElement | null;
    (focusables(root.current)[0] ?? root.current)?.focus();
    const key = (event: KeyboardEvent) => {
      if (event.key === "Escape" && escape.current) {
        event.preventDefault();
        escape.current();
        return;
      }
      if (event.key !== "Tab") return;
      const items = focusables(root.current);
      if (!items.length) {
        event.preventDefault();
        return;
      }
      const first = items[0],
        last = items[items.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };
    document.addEventListener("keydown", key);
    return () => {
      document.removeEventListener("keydown", key);
      previous?.focus?.();
    };
  }, [active, root]);
}

export type Orientation = "horizontal" | "vertical";

/**
 * The index a roving-focus group moves to for `key`, or null when the key does not
 * move focus. Arrows wrap; Home and End jump to the ends; disabled items are skipped.
 */
export function rovingIndex(
  key: string,
  current: number,
  disabled: boolean[],
  orientation: Orientation,
): number | null {
  const count = disabled.length;
  if (!count || disabled.every(Boolean)) return null;
  const next = orientation === "horizontal" ? "ArrowRight" : "ArrowDown";
  const previous = orientation === "horizontal" ? "ArrowLeft" : "ArrowUp";
  const step = (from: number, direction: 1 | -1) => {
    let index = from;
    do index = (index + direction + count) % count;
    while (disabled[index]);
    return index;
  };
  if (key === next) return step(current, 1);
  if (key === previous) return step(current, -1);
  if (key === "Home") return step(-1, 1);
  if (key === "End") return step(count, -1);
  return null;
}

/** State that the caller may control (`value` set) or leave to the component (`initial`). */
export function useControllable<T>(
  value: T | undefined,
  initial: T,
  onChange?: (value: T) => void,
): [T, (value: T) => void] {
  const [own, setOwn] = useState(initial);
  const controlled = value !== undefined;
  const set = useCallback(
    (next: T) => {
      if (!controlled) setOwn(next);
      onChange?.(next);
    },
    [controlled, onChange],
  );
  return [controlled ? (value as T) : own, set];
}

/** Calls `onOutside` for a pointer press outside every element in `refs` while `active`. */
export function useOutsidePress(
  refs: RefObject<HTMLElement | null>[],
  active: boolean,
  onOutside: () => void,
) {
  const outside = useRef(onOutside);
  outside.current = onOutside;
  useEffect(() => {
    if (!active) return;
    const press = (event: PointerEvent | MouseEvent) => {
      const target = event.target as Node;
      if (!refs.some((ref) => ref.current?.contains(target))) outside.current();
    };
    document.addEventListener("mousedown", press);
    return () => document.removeEventListener("mousedown", press);
  }, [active, refs]);
}
