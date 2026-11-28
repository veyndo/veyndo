/** Brand theme cases: logo → derived accessible theme → side-by-side preview → applied only to the private draft; motion per intensity in the real frame. */
import {
  render,
  screen,
  fireEvent,
  within,
  waitFor,
  expect,
  it,
  vi,
} from "../../../../frontend/tests/unit/package-test-support";
import { parseBrandTheme } from "../../../../frontend/src/kit/tokens";
import Canvas from "../studio/StorefrontCanvas";
import BrandThemeDialog from "../studio/BrandThemeDialog";
import AppearanceControls from "../studio/AppearanceControls";
import { defaultTheme, type Theme } from "../model";
import fixture from "./brand-theme-fixture.json";
import { LocaleScope } from "../../../../frontend/src/shared/i18n/i18n";
import type { ReactNode } from "react";
/** Cases read the English vocabulary; the test DOM starts in German. */
const en = (ui: ReactNode) => <LocaleScope locale="en-GB">{ui}</LocaleScope>;

vi.stubGlobal(
  "ResizeObserver",
  class {
    observe() {}
    disconnect() {}
  },
);
const products = [
  {
    id: "coat",
    name: "Actual coat",
    media: [{ id: "m1", url: "/media/demo/coat.png", view: "front" }],
    price: 249,
    currency: "EUR",
  },
];
/** An orange disc on white, as the decoded logo pixels. */
const orangeLogo = () => {
  const size = 32,
    data = new Uint8ClampedArray(size * size * 4);
  for (let y = 0; y < size; y++)
    for (let x = 0; x < size; x++)
      data.set(
        (x - 16) ** 2 + (y - 16) ** 2 < 196
          ? [240, 110, 20, 255]
          : [255, 255, 255, 255],
        (y * size + x) * 4,
      );
  return { data, width: size, height: size, weight: 2 };
};
const companyRequest = () =>
  vi.fn(async (path: string, _body?: unknown, _method?: string) =>
    path === "/store-api/company"
      ? { data: { brandName: "Hafen", logoId: "logo1" } }
      : path.startsWith("/api/settings/company-logo/")
        ? { dataUrl: "data:image/png;base64,AAAA" }
        : {},
  );

it.each(["off", "subtle", "expressive"] as const)(
  "renders the real storefront frame with motion %s and the brand tokens",
  async (motion) => {
    const brand = fixture.vivid as unknown as NonNullable<Theme["brand"]>;
    render(
      en(
        <Canvas
          request={companyRequest()}
          workspace="own-shop"
          channel="default"
          language="en-GB"
          device="desktop"
          products={products}
          appearance={{ ...defaultTheme, motion, brand }}
          onHome={() => {}}
        />,
      ),
    );
    const frame = screen.getByTitle("Preview") as HTMLIFrameElement;
    fireEvent.load(frame);
    await waitFor(() =>
      expect(frame.contentDocument!.querySelector(".shop")).not.toBeNull(),
    );
    const shop = frame.contentDocument!.querySelector<HTMLElement>(".shop")!;
    expect(shop.dataset.motion).toBe(motion);
    expect(shop.style.getPropertyValue("--vy-color-accent")).toBe(
      brand.colors.accent,
    );
    expect(shop.style.getPropertyValue("--veyndo-blue-text")).toBe(
      "var(--vy-color-accent)",
    );
    expect(shop.style.getPropertyValue("--vy-radius-lg")).toBe("28px");
    expect(shop.querySelector(".shop-hero.vy-enter")).not.toBeNull();
  },
);

it("derives a theme from the company logo, compares it and applies it to the draft only", async () => {
  const request = companyRequest(),
    onApply = vi.fn(),
    sample = vi.fn(async () => orangeLogo());
  render(
    en(
      <BrandThemeDialog
        request={request}
        products={products}
        channel="default"
        language="en-GB"
        workspace="own-shop"
        current={defaultTheme}
        sample={sample}
        onApply={onApply}
        onClose={() => {}}
      />,
    ),
  );
  const logo = await screen.findByRole("checkbox", { name: "Company logo" });
  expect(logo).toBeChecked();
  // A product image is offered but not chosen until the merchant ticks it.
  expect(
    screen.getByRole("checkbox", { name: "Product image" }),
  ).not.toBeChecked();
  expect(screen.getByRole("button", { name: "Apply to draft" })).toBeDisabled();
  expect(screen.getAllByTitle("Preview")).toHaveLength(1);
  fireEvent.click(screen.getByRole("button", { name: "Create theme" }));
  expect(
    await screen.findByText("All 17 colour pairs meet WCAG AA contrast."),
  ).toBeVisible();
  expect(sample).toHaveBeenCalledWith(
    expect.objectContaining({ id: "logo:logo1", kind: "logo" }),
  );
  // Current and proposed designs side by side.
  expect(screen.getByText("Current design")).toBeVisible();
  expect(screen.getByText("Brand theme")).toBeVisible();
  expect(screen.getAllByTitle("Preview")).toHaveLength(2);
  fireEvent.change(screen.getByRole("combobox", { name: "Typography" }), {
    target: { value: "crafted" },
  });
  fireEvent.change(screen.getByRole("combobox", { name: "Motion" }), {
    target: { value: "off" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Apply to draft" }));
  const theme = onApply.mock.calls[0][0] as Theme;
  expect(theme.motion).toBe("off");
  expect(theme.brand!.fonts).toEqual({
    pairing: "crafted",
    heading: "slab",
    body: "humanist",
  });
  expect(theme.brand!.radius).toBe("round");
  expect(theme.brand!.source).toEqual(["logo:logo1"]);
  expect(theme.accent).toBe(theme.brand!.colors.accent);
  expect(parseBrandTheme(theme.brand)).toEqual({ theme: theme.brand });
  // Only reads: nothing was saved or published by the dialog.
  for (const [, body, method] of request.mock.calls)
    expect(body === undefined && (method ?? "GET") === "GET").toBe(true);
});

it("asks for an image and reports unreadable ones without touching the draft", async () => {
  const onApply = vi.fn();
  const request = vi.fn(async () => ({ data: {} }));
  render(
    en(
      <BrandThemeDialog
        request={request}
        products={[]}
        channel="default"
        language="de-DE"
        workspace="own-shop"
        current={defaultTheme}
        sample={vi.fn(async () => {
          throw new Error("decode");
        })}
        onApply={onApply}
        onClose={() => {}}
      />,
    ),
  );
  fireEvent.click(screen.getByRole("button", { name: "Create theme" }));
  expect(await screen.findByText("Choose at least one image.")).toBeVisible();
  const file = new File(["x"], "logo.png", { type: "image/png" });
  const svg = new File(["<svg/>"], "logo.svg", { type: "image/svg+xml" });
  vi.stubGlobal("URL", {
    ...URL,
    createObjectURL: () => "blob:x",
    revokeObjectURL: () => {},
  });
  fireEvent.change(screen.getByLabelText("Upload images"), {
    target: { files: [file, svg] },
  });
  expect(
    screen.getByText("An image could not be read. Use PNG, JPEG or WebP."),
  ).toBeVisible();
  expect(
    screen.getByRole("checkbox", { name: "Uploaded image" }),
  ).toBeChecked();
  fireEvent.click(screen.getByRole("button", { name: "Create theme" }));
  await waitFor(() =>
    expect(
      screen.getByText("An image could not be read. Use PNG, JPEG or WebP."),
    ).toBeVisible(),
  );
  expect(onApply).not.toHaveBeenCalled();
  vi.unstubAllGlobals();
  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe() {}
      disconnect() {}
    },
  );
});

it("keeps brand and motion on the home page and removes a brand back to inheritance", () => {
  const onChange = vi.fn();
  const brand = fixture.vivid as unknown as NonNullable<Theme["brand"]>;
  const context = {
    request: companyRequest(),
    products,
    channel: "default",
    language: "en-GB",
    workspace: "own-shop",
  };
  const { rerender } = render(
    en(<AppearanceControls value={{}} open onChange={onChange} />),
  );
  expect(
    screen.getByText(
      "Brand themes and motion apply to the whole shop and are set on the home page.",
    ),
  ).toBeVisible();
  rerender(
    en(
      <AppearanceControls
        value={{}}
        open
        onChange={onChange}
        brand={context}
      />,
    ),
  );
  expect(
    screen.getByRole("button", { name: "Create theme from brand" }),
  ).toBeVisible();
  fireEvent.change(screen.getByRole("combobox", { name: "Motion" }), {
    target: { value: "expressive" },
  });
  expect(onChange).toHaveBeenLastCalledWith({ motion: "expressive" });
  rerender(
    en(
      <AppearanceControls
        value={{ brand, accent: brand.colors.accent, motion: "subtle" }}
        open
        onChange={onChange}
        brand={context}
      />,
    ),
  );
  expect(screen.getByText("Brand theme active")).toBeVisible();
  expect(screen.queryByLabelText("Accent")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Edit brand theme" }));
  expect(
    screen.getByRole("dialog", { name: "Theme from your brand" }),
  ).toBeVisible();
  fireEvent.click(
    within(screen.getByRole("dialog")).getByRole("button", { name: "Cancel" }),
  );
  fireEvent.click(screen.getByRole("button", { name: "Remove brand theme" }));
  expect(onChange).toHaveBeenLastCalledWith(
    expect.objectContaining({ brand: null, accent: null, motion: "subtle" }),
  );
});
