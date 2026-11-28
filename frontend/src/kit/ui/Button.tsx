/** Buttons: one tone and size vocabulary, busy state announced, icon-only buttons require an accessible label. */
import {
  forwardRef,
  type AnchorHTMLAttributes,
  type ButtonHTMLAttributes,
  type ReactNode,
} from "react";
import { Spinner } from "./Feedback";
import type { ButtonDocumentProps } from "./props.generated";

/** Tone and size come from the component catalog's generated types. */
export type ButtonTone = NonNullable<ButtonDocumentProps["tone"]>;
export type ButtonSize = NonNullable<ButtonDocumentProps["size"]>;

export type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  tone?: ButtonTone;
  size?: ButtonSize;
  /** Disables the button, sets aria-busy and shows a spinner before the label. */
  busy?: boolean;
  icon?: ReactNode;
};

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  function Button(
    {
      tone = "secondary",
      size = "md",
      busy = false,
      icon,
      className,
      children,
      type = "button",
      disabled,
      ...rest
    },
    ref,
  ) {
    return (
      <button
        ref={ref}
        type={type}
        className={["vy-button", className].filter(Boolean).join(" ")}
        data-tone={tone}
        data-size={size}
        disabled={disabled || busy}
        aria-busy={busy || undefined}
        {...rest}
      >
        {busy ? <Spinner decorative /> : icon}
        {children}
      </button>
    );
  },
);

export type IconButtonProps = Omit<ButtonProps, "children" | "icon"> & {
  /** The accessible name; icon-only buttons have no visible text. */
  label: string;
  icon: ReactNode;
};

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(
  function IconButton({ label, icon, className, ...rest }, ref) {
    return (
      <Button
        ref={ref}
        aria-label={label}
        title={label}
        className={["vy-icon-button", className].filter(Boolean).join(" ")}
        icon={icon}
        {...rest}
      />
    );
  },
);

export type LinkButtonProps = AnchorHTMLAttributes<HTMLAnchorElement> & {
  tone?: ButtonTone;
  size?: ButtonSize;
};

/** A link that looks like a button: navigation keeps link semantics. */
export const LinkButton = forwardRef<HTMLAnchorElement, LinkButtonProps>(
  function LinkButton(
    { tone = "secondary", size = "md", className, ...rest },
    ref,
  ) {
    return (
      <a
        ref={ref}
        className={["vy-button", className].filter(Boolean).join(" ")}
        data-tone={tone}
        data-size={size}
        {...rest}
      />
    );
  },
);
