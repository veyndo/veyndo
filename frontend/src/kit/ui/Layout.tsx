/** Layout atoms: cards and stacks whose spacing comes only from the token scale. */
import type { ElementType, HTMLAttributes, ReactNode } from "react";
import type { SpaceToken } from "../tokens";

export function Stack({
  gap = "3",
  direction = "column",
  align,
  wrap = false,
  as: Tag = "div",
  children,
  className,
  ...rest
}: HTMLAttributes<HTMLElement> & {
  gap?: SpaceToken;
  direction?: "row" | "column";
  align?: "start" | "center" | "end" | "stretch";
  wrap?: boolean;
  as?: ElementType;
  children?: ReactNode;
}) {
  return (
    <Tag
      className={["vy-stack", className].filter(Boolean).join(" ")}
      data-direction={direction}
      data-align={align}
      data-wrap={wrap || undefined}
      style={{ gap: `var(--vy-space-${gap})` }}
      {...rest}
    >
      {children}
    </Tag>
  );
}

export function Card({
  title,
  actions,
  children,
  as: Tag = "section",
}: {
  title?: ReactNode;
  actions?: ReactNode;
  children?: ReactNode;
  as?: ElementType;
}) {
  return (
    <Tag className="vy-card">
      {(title || actions) && (
        <header>
          {title && <h3>{title}</h3>}
          {actions}
        </header>
      )}
      {children}
    </Tag>
  );
}
