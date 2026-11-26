/** Modal dialog: labelled by its title, focus contained and restored, Escape and the backdrop dismiss unless prevented. */
import {
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
import { useFocusTrap } from "./primitives";

export function Dialog({
  title,
  description,
  children,
  footer,
  onClose,
  dismissible = true,
  container,
}: {
  title: ReactNode;
  description?: ReactNode;
  children?: ReactNode;
  /** Actions, rendered after the content. */
  footer?: ReactNode;
  onClose: () => void;
  /** When false, Escape and the backdrop do nothing; the footer must offer a way out. */
  dismissible?: boolean;
  /**
   * Where the dialog is mounted. Defaults to the nearest open native dialog or themed
   * container around the place it is rendered, so a dark workspace keeps its theme,
   * else the body.
   */
  container?: Element | null;
}) {
  const id = useId(),
    root = useRef<HTMLDivElement>(null),
    anchor = useRef<HTMLSpanElement>(null);
  const [host, setHost] = useState<Element | null>(container ?? null);
  useLayoutEffect(() => {
    setHost(
      container ??
        anchor.current?.parentElement?.closest("dialog[open],[data-theme]") ??
        document.body,
    );
  }, [container]);
  useFocusTrap(root, host !== null, dismissible ? onClose : undefined);
  const anchorElement = <span ref={anchor} hidden />;
  if (!host) return anchorElement;
  return (
    <>
      {anchorElement}
      {createPortal(
        <div
          className="vy-dialog-backdrop"
          onMouseDown={(event) => {
            if (dismissible && event.target === event.currentTarget) onClose();
          }}
        >
          <div
            ref={root}
            className="vy-dialog"
            role="dialog"
            aria-modal="true"
            aria-labelledby={`${id}-title`}
            aria-describedby={description ? `${id}-description` : undefined}
            tabIndex={-1}
          >
            <h2 id={`${id}-title`}>{title}</h2>
            {description && <p id={`${id}-description`}>{description}</p>}
            {children && <div className="vy-dialog-content">{children}</div>}
            {footer && <footer>{footer}</footer>}
          </div>
        </div>,
        host,
      )}
    </>
  );
}
