/** Status display: badges, spinners, notices and empty states with the matching live-region roles. */
import type { ReactNode } from "react";

export type Tone = "neutral" | "accent" | "positive" | "warning" | "danger";

export function Badge({
  tone = "neutral",
  children,
}: {
  tone?: Tone;
  children: ReactNode;
}) {
  return (
    <span className="vy-badge" data-tone={tone}>
      {children}
    </span>
  );
}

/** A spinner; with a label it is a polite status, decorative ones are hidden from assistive technology. */
export function Spinner({
  label,
  decorative = false,
}: {
  label?: string;
  decorative?: boolean;
}) {
  if (decorative || !label)
    return <span className="vy-spinner" aria-hidden="true" />;
  return (
    <span role="status" className="vy-spinner-status">
      <span className="vy-spinner" aria-hidden="true" />
      <span className="vy-visually-hidden">{label}</span>
    </span>
  );
}

/**
 * A notice. Danger is an alert (announced at once); other tones are polite status
 * messages. `action` renders after the text, for example a retry button.
 */
export function Notice({
  tone = "neutral",
  title,
  children,
  action,
}: {
  tone?: Tone;
  title?: ReactNode;
  children?: ReactNode;
  action?: ReactNode;
}) {
  return (
    <div
      className="vy-notice"
      data-tone={tone}
      role={tone === "danger" ? "alert" : "status"}
    >
      <div>
        {title && <strong>{title}</strong>}
        {children && <div>{children}</div>}
      </div>
      {action}
    </div>
  );
}

export function EmptyState({
  title,
  children,
  action,
}: {
  title: ReactNode;
  children?: ReactNode;
  action?: ReactNode;
}) {
  return (
    <section className="vy-empty">
      <h3>{title}</h3>
      {children && <p>{children}</p>}
      {action}
    </section>
  );
}

/** Text only for assistive technology. */
export function VisuallyHidden({ children }: { children: ReactNode }) {
  return <span className="vy-visually-hidden">{children}</span>;
}
