/** Brand consistency across frames: the shop's brand tokens reach MCP App views through host context, and only safe token values are applied. */
import { act, render } from "@testing-library/react";
import { expect, it } from "vitest";
import { View } from "../../src/kit/storefront/mcp-app";
import { appStyleVariables } from "../../src/shared/apps/conversation-app-theme";
import { brandVariables, safeThemeVariables } from "../../src/kit/tokens";
import fixture from "../../../extensions/apps/site-builder/tests/brand-theme-fixture.json";
import type { BrandTheme } from "../../src/kit/tokens";

const brand = fixture.vivid as BrandTheme;

it("reads the brand overrides of the surrounding shop for the host context", () => {
  const shop = document.createElement("div");
  shop.className = "shop";
  const frame = shop.appendChild(document.createElement("iframe"));
  expect(appStyleVariables(frame)).toBeUndefined();
  for (const [name, value] of Object.entries(brandVariables(brand)))
    shop.style.setProperty(name, value);
  const sent = appStyleVariables(frame)!;
  expect(sent["--vy-color-accent"]).toBe(brand.colors.accent);
  // Only kit tokens travel; the Studio palette aliases stay in the host.
  expect(Object.keys(sent).every((k) => k.startsWith("--vy-"))).toBe(true);
  expect(appStyleVariables(null)).toBeUndefined();
  expect(appStyleVariables(document.createElement("iframe"))).toBeUndefined();
});

it("keeps only plain --vy-* values from another frame", () => {
  expect(
    safeThemeVariables({
      "--vy-color-accent": "#b14d00",
      "--vy-font-family": 'Seravek, "Gill Sans Nova", sans-serif',
      "--vy-radius-lg": "28px",
      "--vy-color-bg": "url(https://evil.example/x)",
      "--vy-color-text": "red; background: url(x)",
      "--other": "#000000",
      "--vy-space-4": 12,
    }),
  ).toEqual({
    "--vy-color-accent": "#b14d00",
    "--vy-font-family": 'Seravek, "Gill Sans Nova", sans-serif',
    "--vy-radius-lg": "28px",
  });
  expect(safeThemeVariables(null)).toEqual({});
  expect(safeThemeVariables(["--vy-color-bg"])).toEqual({});
});

it("applies the shop's brand tokens to the storefront parts view", async () => {
  let listener: (message: Record<string, unknown>) => void = () => {};
  render(
    <View
      send={() => {}}
      subscribe={(next) => {
        listener = next;
        return () => {};
      }}
    />,
  );
  await act(async () =>
    listener({
      jsonrpc: "2.0",
      id: "veyndo-init",
      result: {
        hostContext: {
          theme: "light",
          styles: {
            variables: {
              "--vy-color-accent": brand.colors.accent,
              "--vy-color-bg": "url(x)",
            },
          },
        },
      },
    }),
  );
  const root = document.documentElement.style;
  expect(root.getPropertyValue("--vy-color-accent")).toBe(brand.colors.accent);
  expect(root.getPropertyValue("--vy-color-bg")).toBe("");
});
