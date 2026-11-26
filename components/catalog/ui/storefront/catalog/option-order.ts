/** Display order of variant option values: apparel sizes small to large, numbers ascending, other values as returned. */
const sizes = ["XXS", "XS", "S", "M", "L", "XL", "XXL", "XXXL"];
const rank = (value: string) => {
  const size = sizes.indexOf(value.trim().toUpperCase());
  if (size >= 0) return size;
  const number = Number(value.replace(",", "."));
  return Number.isFinite(number) ? number : undefined;
};
/** Variants page by ID, so "M, L, S" can arrive in that order; only fully rankable groups are reordered. */
export function orderOptionValues(values: string[]): string[] {
  const ranks = values.map(rank);
  if (ranks.some((r) => r === undefined)) return values;
  const sizesOnly = values.every((v) => sizes.includes(v.trim().toUpperCase()));
  const numbersOnly = values.every(
    (v) => !sizes.includes(v.trim().toUpperCase()),
  );
  if (!sizesOnly && !numbersOnly) return values;
  return values
    .map((value, index) => ({ value, index, rank: ranks[index]! }))
    .sort((a, b) => a.rank - b.rank || a.index - b.index)
    .map((v) => v.value);
}
