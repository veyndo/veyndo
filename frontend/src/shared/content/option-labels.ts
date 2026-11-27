/** Variant option value names resolved by the server (`extra.optionLabels`); the value key is shown only when no language names it. */
type Labelled = {
  options?: Record<string, string>;
  extra?: { optionLabels?: Record<string, string> } | null;
};

/** The name of `value` in `group`, taken from the first product of the family that has that value. */
export function optionLabel(
  products: Labelled[],
  group: string,
  value: string,
): string {
  for (const p of products)
    if (p.options?.[group] === value) {
      const label = p.extra?.optionLabels?.[group];
      if (label) return label;
    }
  return value;
}

/** A product's option values for display, e.g. "Camel / L". */
export function optionText(product: Labelled): string {
  return Object.entries(product.options ?? {})
    .map(([group, value]) => optionLabel([product], group, value))
    .join(" / ");
}
