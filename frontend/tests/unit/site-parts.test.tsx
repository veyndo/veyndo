/** Site-builder parts: the committed server schema matches the kit contracts; placed parts resolve live products. */
import { readFileSync, writeFileSync } from "node:fs";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import {
  SitePart,
  newPartRef,
  partProducts,
  serverSchemas,
  sitePart,
} from "../../../extensions/apps/site-builder/site-parts";
import { newBlock } from "../../../extensions/apps/site-builder/model";
import { selectedProducts } from "../../../extensions/apps/site-builder/usePageProducts";

// Tests run from frontend/.
const file = "../extensions/apps/site-builder/part-schemas.json";
const generated = `${JSON.stringify(serverSchemas(), null, 2)}\n`;

describe("server part schemas", () => {
  it("match the kit contracts (WRITE_PART_SCHEMAS=1 regenerates)", () => {
    if (process.env.WRITE_PART_SCHEMAS) writeFileSync(file, generated);
    // Compared as data: Prettier owns the file layout.
    expect(JSON.parse(readFileSync(file, "utf8"))).toEqual(serverSchemas());
  });
});

const product = {
  id: "bag",
  name: "Everyday bag",
  price: 169,
  currency: "EUR",
  stock: 4,
  media: [],
};
const block = (id: string, config: Record<string, unknown>) => ({
  ...newBlock("part", "en-GB", "Part"),
  part: { ...newPartRef(sitePart(id)!), config },
});

describe("placed parts", () => {
  it("start from contract defaults and expose their products", () => {
    expect(newPartRef(sitePart("product-card")!)).toEqual({
      id: "product-card",
      version: 1,
      config: {},
    });
    const b = block("product-card", { productId: "bag" });
    expect(partProducts(b)).toEqual(["bag"]);
    expect(
      selectedProducts({
        content: {
          blocks: [
            b,
            { ...newBlock("products", "en-GB", "P"), products: ["shoe"] },
          ],
        },
      } as never),
    ).toEqual(["bag", "shoe"]);
  });

  it("render with live product data and pass the quantity to the host", () => {
    const onAdd = vi.fn();
    const { rerender, container } = render(
      <SitePart
        block={block("product-summary", { productId: "bag" })}
        products={[product]}
        language="en-GB"
        onAdd={onAdd}
      />,
    );
    fireEvent.change(screen.getByRole("spinbutton"), {
      target: { value: "3" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Add to cart" }));
    expect(onAdd).toHaveBeenCalledWith("bag", 3);
    rerender(
      <SitePart
        block={block("product-card", { productId: "gone" })}
        products={[product]}
        language="en-GB"
      />,
    );
    expect(container).toBeEmptyDOMElement();
    rerender(
      <SitePart
        block={block("product-card", { productId: 7 })}
        products={[product]}
        language="en-GB"
      />,
    );
    expect(container).toBeEmptyDOMElement();
  });
});
