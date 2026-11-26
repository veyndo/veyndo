/** The Studio registry: validates extensions once and answers route, navigation and slot questions in registration order. */
import { allows } from "./define";
import {
  contributionLocations,
  type Contribution,
  type NavigationGroup,
  type SlotLocation,
  type StudioExtension,
  type StudioRoute,
} from "./types";

export const navigationGroups: NavigationGroup[] = [
  "intelligence",
  "commerce",
  "experiences",
  "workspace",
];

export function createStudioRegistry<Id extends string>(
  extensions: StudioExtension[],
) {
  const routes: StudioRoute<Id>[] = [];
  const contributions: (Contribution & { owner: string; index: number })[] = [];
  const seen = new Set<string>();
  for (const extension of extensions) {
    if (seen.has(extension.id))
      throw Error(`Duplicate Studio extension ${extension.id}`);
    seen.add(extension.id);
    for (const route of extension.routes ?? []) {
      if (routes.some((known) => known.id === route.id))
        throw Error(`Duplicate Studio route ${route.id}`);
      if (!route.rights.any.length)
        throw Error(`Studio route ${route.id} admits nobody`);
      routes.push(route as StudioRoute<Id>);
    }
    for (const contribution of extension.contributions ?? []) {
      if (
        !contributionLocations[contribution.kind].includes(
          contribution.location,
        )
      )
        throw Error(
          `${contribution.kind} ${contribution.id} cannot use ${contribution.location}`,
        );
      if (contributions.some((known) => known.id === contribution.id))
        throw Error(`Duplicate Studio contribution ${contribution.id}`);
      contributions.push({
        ...contribution,
        owner: extension.id,
        index: contributions.length,
      });
    }
  }
  const byId = new Map(routes.map((route) => [route.id as string, route]));
  return {
    routes,
    route: (id: Id) => byId.get(id),
    canOpen: (id: Id, granted: string[]) => {
      const route = byId.get(id);
      return !!route && allows(route.rights, granted);
    },
    /** The first route the rights open, in registration order. */
    firstRoute: (granted: string[]) =>
      routes.find((route) => allows(route.rights, granted))?.id,
    /** Sidebar groups in fixed order, each with the routes the rights open. */
    navigation: (granted: string[]) =>
      navigationGroups
        .map((group) => ({
          group,
          routes: routes
            .filter(
              (route) => route.group === group && allows(route.rights, granted),
            )
            .sort((a, b) => (a.position ?? 0) - (b.position ?? 0)),
        }))
        .filter((entry) => entry.routes.length),
    /** Contributions to `location` the rights admit, ordered. */
    contributions: <L extends SlotLocation>(location: L, granted: string[]) =>
      contributions
        .filter(
          (contribution) =>
            contribution.location === location &&
            allows(contribution.rights, granted),
        )
        .sort(
          (a, b) => (a.order ?? 0) - (b.order ?? 0) || a.index - b.index,
        ) as unknown as (Contribution<L> & { owner: string })[],
  };
}

export type StudioRegistry<Id extends string> = ReturnType<
  typeof createStudioRegistry<Id>
>;
