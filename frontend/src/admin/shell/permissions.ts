/** Presentation admission from the rights each Studio extension declares. API/domain checks remain authoritative. */
import type { Tab } from "./navigation";
import { studioRegistry } from "./registry/first-party";

export function canOpenStudio(tab: Tab, rights: string[]) {
  return studioRegistry.canOpen(tab, rights);
}
export function firstStudioTab(rights: string[]): Tab | undefined {
  return studioRegistry.firstRoute(rights);
}
