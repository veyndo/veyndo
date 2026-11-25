/** Declaring and admitting extensions: identity helpers that keep descriptor types, and the shared rights rule. */
import type {
  Contribution,
  Rights,
  SlotLocation,
  StudioExtension,
} from "./types";

export const defineStudioExtension = (extension: StudioExtension) => extension;

/** Keeps the location-specific context type of one contribution. */
export const contribute = <L extends SlotLocation>(
  contribution: Contribution<L>,
) => contribution as unknown as Contribution;

/** True when `granted` holds every `all` right and at least one `any` right. */
export function allows(rights: Rights | undefined, granted: string[]) {
  if (!rights) return true;
  return (
    (rights.all ?? []).every((right) => granted.includes(right)) &&
    rights.any.some((right) => granted.includes(right))
  );
}
