/** Safe field shapes of catalog props: plain text without markup, admitted URL forms and bounded identifiers. */
import { z } from "zod";

/** Text an author (agent, app or editor) supplies. Rendered as text, never as HTML; markup is refused so it stays visible. */
export const plainText = (max = 500) =>
  z
    .string()
    .max(max)
    .refine((value) => !/<\/?[a-z!][^>]*>/i.test(value), {
      message: "markup is not admitted; send plain text",
    })
    .refine(
      (value) => !/[\u0000-\u0008\u000b\u000c\u000e-\u001f]/.test(value),
      {
        message: "control characters are not admitted",
      },
    );

/** A required, non-empty plain text, such as an accessible name. */
export const name = (max = 200) =>
  plainText(max).refine((value) => value.trim().length > 0, {
    message: "must not be empty",
  });

/** Stable identifiers inside one document: tab ids, menu item ids, field names. */
export const identifier = z
  .string()
  .regex(/^[A-Za-z][A-Za-z0-9_.-]{0,63}$/, "letters, digits, _ . - only");

/**
 * A relative path or an https URL. Schemes such as `javascript:` or `data:`,
 * protocol-relative `//host` and embedded credentials are refused here; the host
 * additionally limits absolute URLs to the origins it admitted.
 */
export const safeUrl = z
  .string()
  .max(2000)
  .regex(
    /^(\/(?![/\\])|https:\/\/)[^\s<>"'`\\]*$/,
    "only relative or https URLs",
  )
  .refine((value) => !/^https:\/\/[^/]*@/.test(value), {
    message: "credentials in URLs are not admitted",
  })
  .meta({ format: "safe-url" });

/** The tone vocabulary shared by badges and notices. */
export const tone = z.enum([
  "neutral",
  "accent",
  "positive",
  "warning",
  "danger",
]);

/** A step of the spacing scale (`--vy-space-*`). */
export const space = z.enum(["1", "2", "3", "4", "5", "6", "8", "10"]);

/** The action an interactive component reports; the host admits action names. */
export const action = z.union([
  identifier,
  z
    .object({
      name: identifier,
      context: z
        .record(
          identifier,
          z.union([z.string().max(500), z.number(), z.boolean()]),
        )
        .optional(),
    })
    .strict(),
]);
