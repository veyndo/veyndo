/** Buttons: one tone and size vocabulary, busy state announced, icon-only buttons require an accessible label. */
import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";
import { Spinner } from "./Feedback";

export type ButtonTone = "primary" | "secondary" | "ghost" | "danger";
export type ButtonSize = "sm" | "md";

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
