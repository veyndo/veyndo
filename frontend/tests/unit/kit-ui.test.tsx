/** kit/ui follows the WAI-ARIA patterns it names: labels bound by id, keyboard movement, focus containment and restoration. */
import {
  act,
  fireEvent,
  render,
  renderHook,
  screen,
  within,
} from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import * as ui from "../../src/kit/ui";
import {
  Badge,
  Button,
  Card,
  Checkbox,
  Dialog,
  EmptyState,
  Field,
  IconButton,
  Menu,
  Notice,
  Select,
  Spinner,
  Stack,
  Switch,
  Tabs,
  TextArea,
  TextInput,
  VisuallyHidden,
  focusables,
  rovingIndex,
  useControllable,
} from "../../src/kit/ui";
import { color, radius, space, tokens } from "../../src/kit/tokens";
import UICatalogue from "../../../components/apps/ui/admin/developer/catalogue/UICatalogue";
import { LocaleProvider } from "../../src/shared/i18n/i18n";
import { entries } from "../../src/kit/catalog";

describe("primitives", () => {
  it("moves a roving index with wrapping, Home/End and disabled items skipped", () => {
    const disabled = [false, true, false, false];
    expect(rovingIndex("ArrowRight", 0, disabled, "horizontal")).toBe(2);
    expect(rovingIndex("ArrowLeft", 0, disabled, "horizontal")).toBe(3);
    expect(rovingIndex("ArrowDown", 3, disabled, "vertical")).toBe(0);
    expect(rovingIndex("ArrowUp", 2, disabled, "vertical")).toBe(0);
    expect(rovingIndex("Home", 3, [true, false, false], "vertical")).toBe(1);
    expect(rovingIndex("End", 0, [false, false, true], "vertical")).toBe(1);
    expect(rovingIndex("ArrowDown", 0, disabled, "horizontal")).toBeNull();
    expect(rovingIndex("x", 0, disabled, "horizontal")).toBeNull();
    expect(rovingIndex("Home", 0, [true, true], "horizontal")).toBeNull();
    expect(rovingIndex("Home", 0, [], "horizontal")).toBeNull();
  });

  it("keeps controlled state with the caller and uncontrolled state inside", () => {
    const change = vi.fn();
    const own = renderHook(() =>
      useControllable<number | undefined>(undefined, 1, change),
    );
    act(() => own.result.current[1](2));
    expect(own.result.current[0]).toBe(2);
    const held = renderHook(() => useControllable<number>(5, 1, change));
    act(() => held.result.current[1](7));
    expect(held.result.current[0]).toBe(5);
    expect(change).toHaveBeenCalledWith(7);
  });

  it("lists focusable elements in order and skips inert or disabled ones", () => {
    const root = document.createElement("div");
    root.innerHTML =
      '<button>a</button><button disabled>b</button><div inert><input></div><a href="#x">c</a><span tabindex="0">d</span><span tabindex="-1">e</span>';
    expect(focusables(root).map((e) => e.textContent)).toEqual(["a", "c", "d"]);
    expect(focusables(null)).toEqual([]);
  });
});

describe("atoms", () => {
  it("marks a busy button and gives icon buttons an accessible name", () => {
    const click = vi.fn();
    render(
      <>
        <Button tone="primary" busy onClick={click}>
          Save
        </Button>
        <IconButton label="Close" icon={<span>×</span>} />
      </>,
    );
    const save = screen.getByRole("button", { name: "Save" });
    expect(save).toBeDisabled();
    expect(save).toHaveAttribute("aria-busy", "true");
    expect(save).toHaveAttribute("data-tone", "primary");
    expect(save).toHaveAttribute("type", "button");
    fireEvent.click(save);
    expect(click).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Close" })).toHaveAttribute(
      "title",
      "Close",
    );
  });

  it("renders status roles: danger notices alert, others are polite", () => {
    render(
      <>
        <Notice tone="danger" title="Failed" action={<Button>Retry</Button>}>
          No answer
        </Notice>
        <Notice tone="positive">Shipped</Notice>
        <Spinner label="Loading" />
        <Spinner decorative />
        <Badge tone="warning">Pending</Badge>
        <EmptyState title="Nothing" action={<Button>Add</Button>}>
          Yet
        </EmptyState>
        <VisuallyHidden>Hidden text</VisuallyHidden>
      </>,
    );
    expect(screen.getByRole("alert")).toHaveTextContent("FailedNo answer");
    expect(screen.getAllByRole("status").map((e) => e.textContent)).toEqual([
      "Shipped",
      "Loading",
    ]);
    expect(screen.getByText("Pending")).toHaveAttribute("data-tone", "warning");
    expect(
      screen.getByRole("heading", { name: "Nothing" }),
    ).toBeInTheDocument();
    expect(screen.getByText("Hidden text")).toHaveClass("vy-visually-hidden");
  });

  it("lays out stacks and cards on the spacing scale", () => {
    const { container } = render(
      <Stack gap="4" direction="row" wrap align="center" as="ul">
        <li>
          <Card title="Title" actions={<Button>Act</Button>}>
            Body
          </Card>
        </li>
        <li>
          <Card>Plain</Card>
        </li>
      </Stack>,
    );
    const stack = container.querySelector("ul")!;
    expect(stack).toHaveStyle({ gap: "var(--vy-space-4)" });
    expect(stack).toHaveAttribute("data-wrap", "true");
    expect(screen.getByRole("heading", { name: "Title" })).toBeInTheDocument();
    expect(screen.getByText("Plain").querySelector("header")).toBeNull();
  });
});

describe("fields", () => {
  it("binds label, hint and error to the control and marks it invalid", () => {
    render(
      <>
        <Field label="Name" hint="Shown to customers" required>
          <TextInput />
        </Field>
        <Field label="Email" error="Invalid address">
          <TextInput aria-describedby="extra" />
        </Field>
        <Field label="Note">
          <TextArea />
        </Field>
        <Field label="Country">
          <Select>
            <option>DE</option>
          </Select>
        </Field>
      </>,
    );
    const name = screen.getByLabelText(/Name/);
    expect(name).toHaveAccessibleDescription("Shown to customers");
    expect(name).not.toHaveAttribute("aria-invalid");
    const email = screen.getByLabelText("Email");
    expect(email).toHaveAttribute("aria-invalid", "true");
    expect(email.getAttribute("aria-describedby")).toMatch(/-error extra$/);
    expect(screen.getByLabelText("Note").tagName).toBe("TEXTAREA");
    expect(screen.getByLabelText("Country").tagName).toBe("SELECT");
  });

  it("works outside a field with an own id", () => {
    render(<TextInput id="free" aria-label="Free" />);
    expect(screen.getByLabelText("Free")).toHaveAttribute("id", "free");
  });

  it("toggles checkboxes and switches", () => {
    const change = vi.fn();
    render(
      <>
        <Checkbox label="Notify" />
        <Switch label="Active" onChange={change} />
        <Switch label="Locked" checked disabled />
      </>,
    );
    fireEvent.click(screen.getByLabelText("Notify"));
    expect(screen.getByLabelText("Notify")).toBeChecked();
    const active = screen.getByRole("switch", { name: "Active" });
    expect(active).toHaveAttribute("aria-checked", "false");
    fireEvent.click(active);
    expect(active).toHaveAttribute("aria-checked", "true");
    expect(change).toHaveBeenCalledWith(true);
    expect(screen.getByRole("switch", { name: "Locked" })).toBeDisabled();
  });
});

describe("dialog", () => {
  it("contains focus, closes on Escape and the backdrop, and restores focus", () => {
    const trigger = document.createElement("button");
    document.body.append(trigger);
    trigger.focus();
    const close = vi.fn();
    const view = render(
      <Dialog
        title="Delete?"
        description="Orders keep their copy."
        onClose={close}
        footer={
          <>
            <button>Cancel</button>
            <button>Delete</button>
          </>
        }
      >
        <p>Body</p>
      </Dialog>,
    );
    const dialog = screen.getByRole("dialog", { name: "Delete?" });
    expect(dialog).toHaveAttribute("aria-modal", "true");
    expect(dialog).toHaveAccessibleDescription("Orders keep their copy.");
    const [cancel, remove] = within(dialog).getAllByRole("button");
    expect(cancel).toHaveFocus();
    fireEvent.keyDown(document, { key: "Tab", shiftKey: true });
    expect(remove).toHaveFocus();
    fireEvent.keyDown(document, { key: "Tab" });
    expect(cancel).toHaveFocus();
    fireEvent.keyDown(document, { key: "Escape" });
    fireEvent.mouseDown(dialog.parentElement!);
    fireEvent.mouseDown(dialog);
    expect(close).toHaveBeenCalledTimes(2);
    view.unmount();
    expect(trigger).toHaveFocus();
    trigger.remove();
  });

  it("ignores Escape and the backdrop when not dismissible and focuses itself without controls", () => {
    const close = vi.fn();
    render(<Dialog title="Working" onClose={close} dismissible={false} />);
    const dialog = screen.getByRole("dialog");
    expect(dialog).toHaveFocus();
    fireEvent.keyDown(document, { key: "Escape" });
    fireEvent.keyDown(document, { key: "Tab" });
    fireEvent.mouseDown(dialog.parentElement!);
    expect(close).not.toHaveBeenCalled();
  });

  it("mounts inside the themed container it is rendered in, or an explicit one", () => {
    const studio = document.createElement("div");
    studio.dataset.theme = "dark";
    const place = document.createElement("section");
    studio.append(place);
    document.body.append(studio);
    const view = render(<Dialog title="Themed" onClose={() => {}} />, {
      container: place,
    });
    expect(studio.querySelector('[role="dialog"]')).not.toBeNull();
    view.unmount();
    const target = document.createElement("div");
    document.body.append(target);
    render(<Dialog title="Placed" onClose={() => {}} container={target} />, {
      container: studio.appendChild(document.createElement("section")),
    });
    expect(target.querySelector('[role="dialog"]')).not.toBeNull();
    studio.remove();
    target.remove();
  });
});

describe("tabs", () => {
  it("moves selection with arrows, Home and End and links panels", () => {
    const change = vi.fn();
    render(
      <Tabs
        label="Order"
        onChange={change}
        items={[
          { id: "a", label: "Overview", panel: "First" },
          { id: "b", label: "Items", panel: "Second", disabled: true },
          { id: "c", label: "History", panel: "Third" },
        ]}
      />,
    );
    const list = screen.getByRole("tablist", { name: "Order" });
    expect(list).toHaveAttribute("aria-orientation", "horizontal");
    const [overview, , history] = screen.getAllByRole("tab");
    expect(overview).toHaveAttribute("aria-selected", "true");
    expect(overview).toHaveAttribute("tabindex", "0");
    expect(history).toHaveAttribute("tabindex", "-1");
    expect(screen.getByRole("tabpanel")).toHaveTextContent("First");
    expect(screen.getByRole("tabpanel")).toHaveAccessibleName("Overview");
    overview.focus();
    fireEvent.keyDown(overview, { key: "ArrowRight" });
    expect(history).toHaveFocus();
    expect(history).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tabpanel")).toHaveTextContent("Third");
    fireEvent.keyDown(history, { key: "Home" });
    expect(overview).toHaveFocus();
    fireEvent.keyDown(overview, { key: "End" });
    expect(history).toHaveFocus();
    fireEvent.keyDown(history, { key: "a" });
    fireEvent.click(overview);
    expect(change.mock.calls.map((c) => c[0])).toEqual(["c", "a", "c", "a"]);
  });

  it("supports vertical orientation and a controlled value", () => {
    render(
      <Tabs
        label="Settings"
        orientation="vertical"
        value="b"
        items={[
          { id: "a", label: "General", panel: "One" },
          { id: "b", label: "Tax", panel: "Two" },
        ]}
      />,
    );
    const tax = screen.getByRole("tab", { name: "Tax" });
    expect(tax).toHaveAttribute("aria-selected", "true");
    fireEvent.keyDown(tax, { key: "ArrowDown" });
    expect(screen.getByRole("tab", { name: "Tax" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });
});

describe("menu", () => {
  const items = (select = vi.fn()) => [
    { id: "copy", label: "Duplicate", onSelect: select },
    { id: "off", label: "Archive", onSelect: select, disabled: true },
    {
      id: "delete",
      label: "Delete",
      onSelect: select,
      tone: "danger" as const,
    },
  ];

  it("opens from the keyboard, moves with arrows, closes on Escape and returns focus", () => {
    render(<Menu label="Actions" items={items()} />);
    const trigger = screen.getByRole("button", { name: "Actions" });
    expect(trigger).toHaveAttribute("aria-haspopup", "menu");
    expect(trigger).toHaveAttribute("aria-expanded", "false");
    fireEvent.keyDown(trigger, { key: "ArrowDown" });
    expect(trigger).toHaveAttribute("aria-expanded", "true");
    const [copy, , remove] = screen.getAllByRole("menuitem");
    expect(copy).toHaveFocus();
    fireEvent.keyDown(copy, { key: "ArrowDown" });
    expect(remove).toHaveFocus();
    expect(remove).toHaveAttribute("data-tone", "danger");
    fireEvent.keyDown(remove, { key: "q" });
    fireEvent.keyDown(remove, { key: "Escape" });
    expect(screen.queryByRole("menu")).toBeNull();
    expect(trigger).toHaveFocus();
    fireEvent.keyDown(trigger, { key: "ArrowUp" });
    expect(screen.getAllByRole("menuitem")[2]).toHaveFocus();
    fireEvent.keyDown(trigger, { key: "Enter" });
  });

  it("selects with a click, closes on Tab and on an outside press", () => {
    const select = vi.fn();
    render(
      <>
        <Menu label="Actions" trigger={<span>⋯</span>} items={items(select)} />
        <p>Outside</p>
      </>,
    );
    const trigger = screen.getByRole("button", { name: "Actions" });
    fireEvent.click(trigger);
    fireEvent.click(screen.getByRole("menuitem", { name: "Duplicate" }));
    expect(select).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("menu")).toBeNull();
    fireEvent.click(trigger);
    fireEvent.click(trigger);
    expect(screen.queryByRole("menu")).toBeNull();
    fireEvent.click(trigger);
    fireEvent.keyDown(screen.getByRole("menu"), { key: "Tab" });
    expect(screen.queryByRole("menu")).toBeNull();
    fireEvent.click(trigger);
    fireEvent.mouseDown(screen.getByText("Outside"));
    expect(screen.queryByRole("menu")).toBeNull();
  });
});

describe("tokens and catalogue", () => {
  it("names token custom properties", () => {
    expect(color("accent")).toBe("var(--vy-color-accent)");
    expect(space("4")).toBe("var(--vy-space-4)");
    expect(radius("md")).toBe("var(--vy-radius-md)");
    expect(Object.keys(tokens.color.dark)).toEqual(
      Object.keys(tokens.color.light),
    );
  });

  it("declares every public component in the component catalog", () => {
    const components = Object.keys(ui).filter((name) => /^[A-Z]/.test(name));
    const declared = entries.map((entry) => entry.name);
    expect(components.filter((name) => !declared.includes(name))).toEqual([]);
  });

  it("renders the gallery page in both themes", () => {
    render(
      <LocaleProvider>
        <UICatalogue />
      </LocaleProvider>,
    );
    expect(
      screen.getByRole("heading", { level: 1, name: "Component gallery" }),
    ).toBeInTheDocument();
    const frames = document.querySelectorAll("[data-gallery-example]");
    expect(
      new Set([...frames].map((e) => e.getAttribute("data-theme"))),
    ).toEqual(new Set(["light", "dark"]));
    expect(document.querySelectorAll("[data-refusal]")).toHaveLength(0);
  });
});

describe("dialog layout", () => {
  it("keeps long tokens inside the dialog so its actions stay visible", async () => {
    const { readFileSync } = await import("node:fs");
    const css = readFileSync("src/kit/ui/ui.css", "utf8");
    const rule = css.match(/\.vy-dialog \{([^}]*)\}/)?.[1] ?? "";
    // A grid column sized by its content let a package hash widen the dialog to
    // 618px of 520px and hid the Install button behind a horizontal scroll.
    expect(rule).toMatch(/grid-template-columns:\s*minmax\(0,\s*1fr\)/);
    expect(rule).toMatch(/overflow-wrap:\s*anywhere/);
  });
});
