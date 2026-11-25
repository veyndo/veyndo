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
  level = 3,
  as: Tag = "section",
  className,
  ...rest
}: Omit<HTMLAttributes<HTMLElement>, "title"> & {
  title?: ReactNode;
  actions?: ReactNode;
  children?: ReactNode;
  /** Heading level of the title, so the card fits the page outline. */
  level?: 2 | 3 | 4;
  as?: ElementType;
}) {
  const Heading = `h${level}` as const;
  return (
    <Tag className={["vy-card", className].filter(Boolean).join(" ")} {...rest}>
      {(title || actions) && (
        <header>
          {title && <Heading>{title}</Heading>}
          {actions}
        </header>
      )}
      {children}
    </Tag>
  );
}
