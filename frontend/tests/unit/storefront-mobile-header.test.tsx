/** The compact storefront header: menu drawer with navigation and settings, header search and bag in one row. */
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import StorefrontHeaderView from "../../src/storefront/shell/StorefrontHeaderView";
import { LocaleProvider } from "../../src/shared/i18n/i18n";

function mount(props: Partial<Parameters<typeof StorefrontHeaderView>[0]>) {
  return render(
    <StorefrontHeaderView displayName="Hafen" count={2} {...props}>
      <select aria-label="Language">
        <option>English</option>
      </select>
    </StorefrontHeaderView>,
    { wrapper: LocaleProvider },
  );
}

it("opens the menu drawer, closes it on Escape and returns focus", async () => {
  const user = userEvent.setup();
  const { container } = mount({});
  const toggle = screen.getByRole("button", { name: "Menu" });
  expect(toggle).toHaveAttribute("aria-expanded", "false");
  await user.click(toggle);
  const header = container.querySelector(".shop-nav")!;
  expect(header).toHaveAttribute("data-menu", "open");
  const drawer = document.getElementById(
    toggle.getAttribute("aria-controls")!,
  )!;
  expect(drawer.querySelector("nav")).toBeTruthy();
  expect(drawer.querySelector("select")).toBeTruthy();
  fireEvent.keyDown(window, { key: "Escape" });
  expect(header).not.toHaveAttribute("data-menu");
  expect(toggle).toHaveFocus();
});

it("closes the drawer when the shopper chooses a destination", async () => {
  const user = userEvent.setup(),
    onAccount = vi.fn();
  const { container } = mount({ onAccount });
  await user.click(screen.getByRole("button", { name: "Menu" }));
  await user.click(screen.getByRole("button", { name: "My account" }));
  expect(onAccount).toHaveBeenCalled();
  expect(container.querySelector(".shop-nav")).not.toHaveAttribute("data-menu");
});

it("searches from the header and keeps the bag count in the bag's name", async () => {
  const user = userEvent.setup(),
    onSearch = vi.fn();
  mount({ onSearch, query: "" });
  expect(screen.getByRole("button", { name: "Bag 2" })).toBeTruthy();
  await user.click(screen.getByRole("button", { name: "Open search" }));
  const field = screen.getByRole("searchbox", { name: "Search products" });
  await user.type(field, " wool coat {Enter}");
  expect(onSearch).toHaveBeenCalledWith("wool coat");
  expect(screen.queryByRole("search")).toBeNull();
});

it("offers no header search where the caller has no search command", () => {
  mount({});
  expect(screen.getByRole("button", { name: "Open search" })).toBeDisabled();
});
