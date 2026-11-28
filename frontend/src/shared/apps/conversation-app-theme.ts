/** Resolve and observe the existing host theme without loading App records or gaining bridge authority. */
export function appTheme(frame: HTMLIFrameElement | null): "light" | "dark" {
  return frame && getComputedStyle(frame).colorScheme === "dark"
    ? "dark"
    : "light";
}
/** The brand token overrides of the surrounding shop (inline `--vy-*` on `.shop`), sent as MCP App host context `styles.variables`. */
export function appStyleVariables(
  frame: HTMLIFrameElement | null,
): Record<string, string> | undefined {
  const shop = frame?.closest<HTMLElement>(".shop");
  if (!shop) return undefined;
  const variables: Record<string, string> = {};
  for (let i = 0; i < shop.style.length; i++) {
    const name = shop.style[i];
    if (name.startsWith("--vy-"))
      variables[name] = shop.style.getPropertyValue(name).trim();
  }
  return Object.keys(variables).length ? variables : undefined;
}
export function observeAppTheme(
  frame: HTMLIFrameElement | null,
  notify: (theme: "light" | "dark") => void,
) {
  const owner = frame?.closest("[data-theme]");
  const observer = new MutationObserver(() => notify(appTheme(frame)));
  if (owner)
    observer.observe(owner, {
      attributes: true,
      attributeFilter: ["data-theme"],
    });
  return () => observer.disconnect();
}
