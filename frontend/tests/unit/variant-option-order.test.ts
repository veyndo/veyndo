/** Variant option values are shown small to large although variants arrive ordered by ID. */
import { expect, it } from "vitest";
import { orderOptionValues } from "../../../components/catalog/ui/storefront/catalog/option-order";

it("orders apparel sizes from small to large", () => {
  expect(orderOptionValues(["M", "L", "S"])).toEqual(["S", "M", "L"]);
  expect(orderOptionValues(["xl", "XS", "m"])).toEqual(["XS", "m", "xl"]);
});

it("orders numeric sizes ascending", () => {
  expect(orderOptionValues(["42", "38", "40,5"])).toEqual(["38", "40,5", "42"]);
});

it("keeps the returned order for colours and mixed groups", () => {
  expect(orderOptionValues(["camel", "black"])).toEqual(["camel", "black"]);
  expect(orderOptionValues(["M", "38", "S"])).toEqual(["M", "38", "S"]);
});
