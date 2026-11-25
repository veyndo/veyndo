/** The generated route catalogue as records, and safe path filling for a route template. */
import { routeSources, type RouteKey } from "./generated/routes";

export type { RouteKey };
export type Route = {
  key: RouteKey;
  method: string;
  path: string;
  source: string;
};

/** Every static route, in catalogue order. */
export const routes: Route[] = (Object.keys(routeSources) as RouteKey[]).map(
  (key) => {
    const space = key.indexOf(" ");
    return {
      key,
      method: key.slice(0, space),
      path: key.slice(space + 1),
      source: routeSources[key],
    };
  },
);

/** The path of a route key, without its method. */
export type RoutePathOf<K extends RouteKey> = K extends `${string} ${infer P}`
  ? P
  : never;

/** The named `{parameters}` of a path template. */
export type PathParams<P extends string> =
  P extends `${string}{${infer Name}}${infer Rest}`
    ? { [Key in Name | keyof PathParams<Rest>]: string | number }
    : Record<never, never>;

/**
 * Fills the `{parameters}` of a path template. Each value is trimmed and encoded;
 * empty values, `.`/`..` and values with `/`, `\`, `?` or `#` are refused so a
 * parameter can never escape its path segment.
 */
export function fillPath(
  template: string,
  params: Record<string, string | number | undefined>,
): string {
  return template.replace(/\{([^}]+)\}/g, (_, name: string) => {
    const value = String(params[name] ?? "").trim();
    if (!value || value === "." || value === ".." || /[/\\?#]/.test(value))
      throw Error(`Invalid parameter: ${name}`);
    return encodeURIComponent(value);
  });
}
