/** Catalog entries of kit/ui: each component's document props, slots, events, accessibility contract and examples. */
import { useState, type ReactNode } from "react";
import { z } from "zod";
import {
  Badge,
  Button,
  Card,
  Checkbox,
  EmptyState,
  Field,
  LinkButton,
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
} from "../ui";
import { defineEntry, type CatalogEntry } from "./define";
import { identifier, name, plainText, safeUrl, space, tone } from "./fields";

const everywhere = ["studio", "storefront", "app", "mcp-app"] as const;
const buttonTone = z.enum(["primary", "secondary", "ghost", "danger"]);
const size = z.enum(["sm", "md"]);
const level = z.union([z.literal(2), z.literal(3), z.literal(4)]);
const changed = z.object({ name: identifier, value: z.string() });
const toggled = z.object({ name: identifier, checked: z.boolean() });

/** A labelled control keeps its own value; the host hears a committed value through `change`. */
function LocalInput({
  multiline,
  props,
  emit,
}: {
  multiline: boolean;
  props: {
    name: string;
    label: string;
    hint?: string;
    error?: string;
    required: boolean;
    value?: string;
    placeholder?: string;
    type?: string;
    rows?: number;
  };
  emit: (event: string, payload?: unknown) => void;
}) {
  const [value, setValue] = useState(props.value ?? "");
  const commit = () => emit("change", { name: props.name, value });
  const shared = {
    name: props.name,
    value,
    required: props.required,
    placeholder: props.placeholder,
    onBlur: commit,
  };
  return (
    <Field
      label={props.label}
      hint={props.hint}
      error={props.error}
      required={props.required}
    >
      {multiline ? (
        <TextArea
          {...shared}
          rows={props.rows}
          onChange={(event) => setValue(event.target.value)}
        />
      ) : (
        <TextInput
          {...shared}
          type={props.type}
          onChange={(event) => setValue(event.target.value)}
          onKeyDown={(event) => event.key === "Enter" && commit()}
        />
      )}
    </Field>
  );
}

const field = {
  name: identifier,
  label: name(),
  hint: plainText(300).optional(),
  error: plainText(300).optional(),
  required: z.boolean().default(false),
  value: plainText(5000).optional(),
  placeholder: plainText(200).optional(),
};

export const uiEntries: CatalogEntry[] = [
  defineEntry({
    name: "Text",
    kind: "content",
    description: "A paragraph of plain text.",
    source: "kit/catalog",
    since: "1.0.0",
    props: z
      .object({
        text: plainText(4000),
        tone: z.enum(["default", "muted"]).default("default"),
      })
      .strict(),
    a11y: {
      name: "none",
      notes: "Rendered as a paragraph; text is never parsed as HTML.",
    },
    surfaces: everywhere,
    render: ({ props }) => (
      <p className="vy-doc-text" data-tone={props.tone}>
        {props.text}
      </p>
    ),
    examples: [
      {
        id: "text",
        state: "default",
        node: (t) => ({ component: "Text", props: { text: t("body") } }),
      },
      {
        id: "muted",
        state: "muted",
        node: (t) => ({
          component: "Text",
          props: { text: t("hint"), tone: "muted" },
        }),
      },
    ],
  }),
  defineEntry({
    name: "Heading",
    kind: "content",
    description:
      "A section heading at level 2, 3 or 4, so the document fits the host's outline.",
    source: "kit/catalog",
    since: "1.0.0",
    props: z.object({ text: name(200), level: level.default(3) }).strict(),
    a11y: {
      role: "heading",
      name: "content",
      notes: "Documents start below the host's page heading (level 1).",
    },
    surfaces: everywhere,
    render: ({ props }) => {
      const Tag = `h${props.level}` as const;
      return <Tag className="vy-doc-heading">{props.text}</Tag>;
    },
    examples: [
      {
        id: "heading",
        state: "level 3",
        node: (t) => ({ component: "Heading", props: { text: t("title") } }),
      },
    ],
  }),
  defineEntry({
    name: "Image",
    kind: "content",
    description:
      "An image from a relative or https URL on an admitted origin, with required alternative text.",
    source: "kit/catalog",
    since: "1.0.0",
    props: z
      .object({
        src: safeUrl,
        alt: plainText(300),
        decorative: z.boolean().default(false),
        width: z.number().int().min(1).max(4000).optional(),
        height: z.number().int().min(1).max(4000).optional(),
      })
      .strict()
      .refine((p) => p.decorative || p.alt.trim().length > 0, {
        message: "alt text is required unless the image is decorative",
        path: ["alt"],
      }),
    a11y: {
      role: "img",
      name: { prop: "alt" },
      notes:
        "Decorative images get an empty alt and are hidden from assistive technology.",
    },
    surfaces: everywhere,
    render: ({ props: p }) => {
      return (
        <img
          className="vy-doc-image"
          src={p.src}
          alt={p.decorative ? "" : p.alt}
          width={p.width}
          height={p.height}
          loading="lazy"
          referrerPolicy="no-referrer"
        />
      );
    },
    examples: [],
  }),
  defineEntry({
    name: "Stack",
    kind: "layout",
    description: "Arranges children in a row or column on the spacing scale.",
    source: "kit/ui#Stack",
    since: "1.0.0",
    props: z
      .object({
        gap: space.default("3"),
        direction: z.enum(["row", "column"]).default("column"),
        align: z.enum(["start", "center", "end", "stretch"]).optional(),
        wrap: z.boolean().default(false),
      })
      .strict(),
    slots: {
      children: {
        description: "The arranged content.",
        accepts: "any",
        max: 50,
      },
    },
    a11y: {
      name: "none",
      notes: "No role; reading order follows document order.",
    },
    surfaces: everywhere,
    render: ({ props, slots }) => (
      <Stack
        gap={props.gap}
        direction={props.direction}
        align={props.align}
        wrap={props.wrap}
      >
        {slots.children}
      </Stack>
    ),
    examples: [
      {
        id: "row",
        state: "row",
        actions: ["save"],
        node: (t) => ({
          component: "Stack",
          props: { direction: "row", gap: "2", align: "center" },
          children: [
            { component: "Badge", props: { text: t("draft") } },
            {
              component: "Button",
              props: { label: t("save"), tone: "primary" },
              action: "save",
            },
          ],
        }),
      },
    ],
  }),
  defineEntry({
    name: "Card",
    kind: "layout",
    description:
      "A titled surface that groups related content, with optional header actions.",
    source: "kit/ui#Card",
    since: "1.0.0",
    props: z
      .object({ title: plainText(200).optional(), level: level.default(3) })
      .strict(),
    slots: {
      children: { description: "The card body.", accepts: "any", max: 50 },
      actions: {
        description: "Header actions such as a badge or a button.",
        accepts: ["Badge", "Button", "LinkButton", "Menu"],
        max: 3,
      },
    },
    a11y: {
      name: { prop: "title" },
      notes: "A section with a heading at the level the document sets.",
    },
    surfaces: everywhere,
    render: ({ props, slots }) => (
      <Card
        title={props.title}
        level={props.level}
        actions={slots.actions.length ? <>{slots.actions}</> : undefined}
      >
        {slots.children}
      </Card>
    ),
    examples: [
      {
        id: "card",
        state: "with actions",
        node: (t) => ({
          component: "Card",
          props: { title: t("card") },
          slots: {
            actions: [
              { component: "Badge", props: { text: t("new"), tone: "accent" } },
            ],
          },
          children: [{ component: "Text", props: { text: t("body") } }],
        }),
      },
    ],
  }),
  defineEntry({
    name: "Button",
    kind: "atom",
    description:
      "Triggers an admitted host action. Without an admitted action it renders disabled.",
    source: "kit/ui#Button",
    since: "1.0.0",
    props: z
      .object({
        label: name(120),
        tone: buttonTone.default("secondary"),
        size: size.default("md"),
        busy: z.boolean().default(false),
        disabled: z.boolean().default(false),
      })
      .strict(),
    events: {
      press: {
        description: "The button was activated by pointer, Enter or Space.",
      },
    },
    a11y: {
      role: "button",
      name: "content",
      keyboard: ["Enter", "Space"],
      notes:
        "Busy sets aria-busy, disables the button and shows a decorative spinner.",
    },
    surfaces: everywhere,
    render: ({ props, emit, bound }) => (
      <Button
        tone={props.tone}
        size={props.size}
        busy={props.busy}
        disabled={props.disabled || !bound("press")}
        onClick={() => emit("press")}
      >
        {props.label}
      </Button>
    ),
    examples: [
      {
        id: "tones",
        state: "tones",
        actions: ["save"],
        node: (t) => ({
          component: "Stack",
          props: { direction: "row", gap: "2", wrap: true },
          children: (["primary", "secondary", "ghost", "danger"] as const).map(
            (tone) => ({
              component: "Button",
              props: {
                label: t(
                  tone === "danger"
                    ? "delete"
                    : tone === "primary"
                      ? "save"
                      : tone === "ghost"
                        ? "more"
                        : "cancel",
                ),
                tone,
              },
              action: "save",
            }),
          ),
        }),
      },
      {
        id: "busy",
        state: "busy",
        actions: ["save"],
        node: (t) => ({
          component: "Button",
          props: { label: t("saving"), tone: "primary", busy: true },
          action: "save",
        }),
      },
      {
        id: "disabled",
        state: "disabled",
        node: (t) => ({
          component: "Button",
          props: { label: t("save"), disabled: true },
        }),
      },
    ],
  }),
  defineEntry({
    name: "IconButton",
    kind: "atom",
    description: "A button with only an icon and a required accessible label.",
    source: "kit/ui#IconButton",
    since: "1.0.0",
    props: z
      .object({
        label: name(120),
        tone: buttonTone.default("secondary"),
        size: size.default("md"),
      })
      .strict(),
    events: { press: { description: "The button was activated." } },
    a11y: {
      role: "button",
      name: { prop: "label" },
      keyboard: ["Enter", "Space"],
      notes: "The label is the accessible name and the tooltip.",
    },
    surfaces: [],
    codeOnly: "Icons are code; documents use Button with a visible label.",
    render: () => null,
    examples: [],
  }),
  defineEntry({
    name: "LinkButton",
    kind: "atom",
    description:
      "Navigation that looks like a button; keeps link semantics. URLs must be relative or on an admitted https origin.",
    source: "kit/ui#LinkButton",
    since: "1.0.0",
    props: z
      .object({
        label: name(120),
        href: safeUrl,
        tone: buttonTone.default("secondary"),
        size: size.default("md"),
      })
      .strict(),
    a11y: {
      role: "link",
      name: "content",
      keyboard: ["Enter"],
      notes: "Absolute links open in a new tab without referrer or opener.",
    },
    surfaces: everywhere,
    render: ({ props }) => {
      const external = props.href.startsWith("https://");
      return (
        <LinkButton
          href={props.href}
          tone={props.tone}
          size={props.size}
          target={external ? "_blank" : undefined}
          rel={external ? "noopener noreferrer" : undefined}
        >
          {props.label}
        </LinkButton>
      );
    },
    examples: [
      {
        id: "link",
        state: "default",
        node: (t) => ({
          component: "LinkButton",
          props: { label: t("details"), href: "/products" },
        }),
      },
    ],
  }),
  defineEntry({
    name: "Badge",
    kind: "atom",
    description: "A short status label.",
    source: "kit/ui#Badge",
    since: "1.0.0",
    props: z.object({ text: name(60), tone: tone.default("neutral") }).strict(),
    a11y: {
      name: "content",
      notes: "Tone is visual only; the text carries the meaning.",
    },
    surfaces: everywhere,
    render: ({ props }) => <Badge tone={props.tone}>{props.text}</Badge>,
    examples: [
      {
        id: "tones",
        state: "tones",
        node: (t) => ({
          component: "Stack",
          props: { direction: "row", gap: "2", wrap: true },
          children: [
            { component: "Badge", props: { text: t("draft") } },
            { component: "Badge", props: { text: t("new"), tone: "accent" } },
            {
              component: "Badge",
              props: { text: t("paid"), tone: "positive" },
            },
            {
              component: "Badge",
              props: { text: t("pending"), tone: "warning" },
            },
            {
              component: "Badge",
              props: { text: t("failed"), tone: "danger" },
            },
          ],
        }),
      },
    ],
  }),
  defineEntry({
    name: "Spinner",
    kind: "atom",
    description:
      "Progress of unknown length. With a label it is a polite status; without one it is decorative.",
    source: "kit/ui#Spinner",
    since: "1.0.0",
    props: z.object({ label: name(120).optional() }).strict(),
    a11y: {
      role: "status",
      name: "content",
      notes:
        "The label is read as hidden status text; motion stops under prefers-reduced-motion.",
    },
    surfaces: everywhere,
    render: ({ props }) => <Spinner label={props.label} />,
    examples: [
      {
        id: "spinner",
        state: "loading",
        node: (t) => ({ component: "Spinner", props: { label: t("loading") } }),
      },
    ],
  }),
  defineEntry({
    name: "VisuallyHidden",
    kind: "content",
    description: "Text only for assistive technology.",
    source: "kit/ui#VisuallyHidden",
    since: "1.0.0",
    props: z.object({ text: name(300) }).strict(),
    a11y: {
      name: "content",
      notes: "Hidden visually, read by screen readers.",
    },
    surfaces: everywhere,
    render: ({ props }) => <VisuallyHidden>{props.text}</VisuallyHidden>,
    examples: [],
  }),
  defineEntry({
    name: "Field",
    kind: "molecule",
    description: "Binds a label, hint and error to one control.",
    source: "kit/ui#Field",
    since: "1.0.0",
    props: z
      .object({
        label: name(),
        hint: plainText(300).optional(),
        error: plainText(300).optional(),
        required: z.boolean().default(false),
      })
      .strict(),
    a11y: {
      name: { prop: "label" },
      notes:
        "The control gets the label by id and the hint or error through aria-describedby.",
    },
    surfaces: [],
    codeOnly:
      "Documents use the labelled controls TextInput, TextArea, Select, Checkbox and Switch, which render their Field.",
    render: () => null,
    examples: [],
  }),
  defineEntry({
    name: "TextInput",
    kind: "atom",
    description:
      "A labelled single-line text field. The value stays in the view until committed.",
    source: "kit/ui#TextInput",
    since: "1.0.0",
    props: z
      .object({
        ...field,
        type: z
          .enum(["text", "email", "number", "search", "tel", "url"])
          .default("text"),
      })
      .strict(),
    events: {
      change: {
        description: "The value was committed (blur or Enter).",
        payload: changed,
      },
    },
    a11y: {
      role: "textbox",
      name: { prop: "label" },
      notes:
        "Errors set aria-invalid and are announced through aria-describedby.",
    },
    surfaces: everywhere,
    render: ({ props, emit }) => (
      <LocalInput multiline={false} props={props} emit={emit} />
    ),
    examples: [
      {
        id: "hint",
        state: "with hint",
        node: (t) => ({
          component: "TextInput",
          props: {
            name: "name",
            label: t("name"),
            hint: t("hint"),
            required: true,
          },
        }),
      },
      {
        id: "invalid",
        state: "invalid",
        node: (t) => ({
          component: "TextInput",
          props: {
            name: "email",
            label: t("email"),
            type: "email",
            error: t("emailError"),
          },
        }),
      },
    ],
  }),
  defineEntry({
    name: "TextArea",
    kind: "atom",
    description: "A labelled multi-line text field.",
    source: "kit/ui#TextArea",
    since: "1.0.0",
    props: z
      .object({ ...field, rows: z.number().int().min(1).max(20).default(3) })
      .strict(),
    events: {
      change: {
        description: "The value was committed (blur).",
        payload: changed,
      },
    },
    a11y: {
      role: "textbox",
      name: { prop: "label" },
      notes: "Multi-line; Enter inserts a line break.",
    },
    surfaces: everywhere,
    render: ({ props, emit }) => (
      <LocalInput multiline props={props} emit={emit} />
    ),
    examples: [
      {
        id: "note",
        state: "default",
        node: (t) => ({
          component: "TextArea",
          props: { name: "note", label: t("note"), rows: 2 },
        }),
      },
    ],
  }),
  defineEntry({
    name: "Select",
    kind: "atom",
    description: "A labelled choice from a short list.",
    source: "kit/ui#Select",
    since: "1.0.0",
    props: z
      .object({
        name: identifier,
        label: name(),
        hint: plainText(300).optional(),
        options: z
          .array(z.object({ value: plainText(200), label: name(200) }).strict())
          .min(1)
          .max(100),
        value: plainText(200).optional(),
      })
      .strict(),
    events: {
      change: { description: "Another option was chosen.", payload: changed },
    },
    a11y: {
      role: "combobox",
      name: { prop: "label" },
      keyboard: ["Arrow keys", "Home", "End"],
      notes: "A native select.",
    },
    surfaces: everywhere,
    render: function SelectEntry({ props, emit }) {
      return (
        <Field label={props.label} hint={props.hint}>
          <Select
            name={props.name}
            defaultValue={props.value}
            onChange={(event) =>
              emit("change", { name: props.name, value: event.target.value })
            }
          >
            {props.options.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </Select>
        </Field>
      );
    },
    examples: [
      {
        id: "country",
        state: "default",
        node: (t) => ({
          component: "Select",
          props: {
            name: "country",
            label: t("country"),
            value: "DE",
            options: [
              { value: "DE", label: t("germany") },
              { value: "ES", label: t("spain") },
            ],
          },
        }),
      },
    ],
  }),
  defineEntry({
    name: "Checkbox",
    kind: "atom",
    description: "A labelled checkbox.",
    source: "kit/ui#Checkbox",
    since: "1.0.0",
    props: z
      .object({
        name: identifier,
        label: name(),
        checked: z.boolean().default(false),
      })
      .strict(),
    events: {
      change: {
        description: "The box was ticked or cleared.",
        payload: toggled,
      },
    },
    a11y: {
      role: "checkbox",
      name: { prop: "label" },
      keyboard: ["Space"],
      notes: "A native checkbox linked to its label.",
    },
    surfaces: everywhere,
    render: ({ props, emit }) => (
      <Checkbox
        name={props.name}
        label={props.label}
        defaultChecked={props.checked}
        onChange={(event) =>
          emit("change", { name: props.name, checked: event.target.checked })
        }
      />
    ),
    examples: [
      {
        id: "terms",
        state: "checked",
        node: (t) => ({
          component: "Checkbox",
          props: { name: "terms", label: t("terms"), checked: true },
        }),
      },
    ],
  }),
  defineEntry({
    name: "Switch",
    kind: "atom",
    description: "An on/off switch for a setting that applies at once.",
    source: "kit/ui#Switch",
    since: "1.0.0",
    props: z
      .object({
        name: identifier,
        label: name(),
        checked: z.boolean().default(false),
        disabled: z.boolean().default(false),
      })
      .strict(),
    events: {
      change: {
        description: "The switch was turned on or off.",
        payload: toggled,
      },
    },
    a11y: {
      role: "switch",
      pattern: "switch",
      name: { prop: "label" },
      keyboard: ["Space", "Enter"],
      notes: "aria-checked follows the state.",
    },
    surfaces: everywhere,
    render: ({ props, emit }) => (
      <Switch
        label={props.label}
        defaultChecked={props.checked}
        disabled={props.disabled}
        onChange={(checked) => emit("change", { name: props.name, checked })}
      />
    ),
    examples: [
      {
        id: "on",
        state: "on",
        node: (t) => ({
          component: "Switch",
          props: { name: "active", label: t("active"), checked: true },
        }),
      },
      {
        id: "disabled",
        state: "disabled",
        node: (t) => ({
          component: "Switch",
          props: { name: "active", label: t("active"), disabled: true },
        }),
      },
    ],
  }),
  defineEntry({
    name: "Notice",
    kind: "molecule",
    description:
      "A message about the current state. Danger is announced at once; other tones are polite.",
    source: "kit/ui#Notice",
    since: "1.0.0",
    props: z
      .object({
        tone: tone.default("neutral"),
        title: plainText(200).optional(),
        text: plainText(2000).optional(),
      })
      .strict(),
    slots: {
      action: {
        description: "One follow-up action, such as retry.",
        accepts: ["Button", "LinkButton"],
        max: 1,
      },
    },
    a11y: {
      role: "alert / status",
      name: "content",
      notes: "Danger uses role=alert, every other tone role=status.",
    },
    surfaces: everywhere,
    render: ({ props, slots }) => (
      <Notice tone={props.tone} title={props.title} action={slots.action[0]}>
        {props.text}
      </Notice>
    ),
    examples: [
      {
        id: "positive",
        state: "positive",
        node: (t) => ({
          component: "Notice",
          props: { tone: "positive", text: t("shipped") },
        }),
      },
      {
        id: "danger",
        state: "danger with action",
        actions: ["retry"],
        node: (t) => ({
          component: "Notice",
          props: {
            tone: "danger",
            title: t("failed"),
            text: t("providerDown"),
          },
          slots: {
            action: [
              {
                component: "Button",
                props: { label: t("retry"), size: "sm" },
                action: "retry",
              },
            ],
          },
        }),
      },
    ],
  }),
  defineEntry({
    name: "EmptyState",
    kind: "molecule",
    description: "Explains why a list or area is empty and what to do next.",
    source: "kit/ui#EmptyState",
    since: "1.0.0",
    props: z
      .object({ title: name(200), text: plainText(1000).optional() })
      .strict(),
    slots: {
      action: {
        description: "The next step.",
        accepts: ["Button", "LinkButton"],
        max: 1,
      },
    },
    a11y: {
      name: "content",
      notes: "A section whose title is a level-3 heading.",
    },
    surfaces: everywhere,
    render: ({ props, slots }) => (
      <EmptyState title={props.title} action={slots.action[0]}>
        {props.text}
      </EmptyState>
    ),
    examples: [
      {
        id: "empty",
        state: "default",
        node: (t) => ({
          component: "EmptyState",
          props: { title: t("empty"), text: t("emptyBody") },
        }),
      },
    ],
  }),
  defineEntry({
    name: "Tabs",
    kind: "molecule",
    description: "Switches between panels of related content.",
    source: "kit/ui#Tabs",
    since: "1.0.0",
    props: z
      .object({
        label: name(),
        orientation: z.enum(["horizontal", "vertical"]).default("horizontal"),
        value: identifier.optional(),
      })
      .strict(),
    slots: {
      children: {
        description: "The panels, one per tab.",
        accepts: ["TabPanel"],
        min: 1,
        max: 12,
      },
    },
    events: {
      change: {
        description: "Another tab was selected.",
        payload: z.object({ id: identifier }),
      },
    },
    a11y: {
      role: "tablist",
      pattern: "tabs",
      name: { prop: "label" },
      keyboard: ["Arrow keys", "Home", "End"],
      notes:
        "Roving focus with automatic activation; panels are linked to their tabs.",
    },
    surfaces: everywhere,
    render: function TabsEntry({ props, slots, slotProps, emit }) {
      const panels = slots.children as ReactNode[];
      return (
        <Tabs
          label={props.label}
          orientation={props.orientation}
          defaultValue={props.value}
          onChange={(id) => emit("change", { id })}
          items={panels.map((panel, index) => {
            const tab = slotProps.children[index] as {
              id: string;
              label: string;
              disabled: boolean;
            };
            return {
              id: tab.id,
              label: tab.label,
              disabled: tab.disabled,
              panel,
            };
          })}
        />
      );
    },
    examples: [
      {
        id: "tabs",
        state: "default",
        node: (t) => ({
          component: "Tabs",
          props: { label: t("navigation") },
          children: [
            {
              component: "TabPanel",
              props: { id: "overview", label: t("overview") },
              children: [{ component: "Text", props: { text: t("shipped") } }],
            },
            {
              component: "TabPanel",
              props: { id: "details", label: t("details") },
              children: [{ component: "Text", props: { text: t("hint") } }],
            },
            {
              component: "TabPanel",
              props: { id: "history", label: t("history"), disabled: true },
            },
          ],
        }),
      },
    ],
  }),
  defineEntry({
    name: "TabPanel",
    kind: "layout",
    description: "One tab and its panel inside Tabs.",
    source: "kit/ui#Tabs",
    since: "1.0.0",
    props: z
      .object({
        id: identifier,
        label: name(120),
        disabled: z.boolean().default(false),
      })
      .strict(),
    slots: {
      children: { description: "The panel content.", accepts: "any", max: 50 },
    },
    within: ["Tabs"],
    a11y: {
      role: "tabpanel",
      name: { prop: "label" },
      notes: "Labelled by its tab; only the selected panel is rendered.",
    },
    surfaces: everywhere,
    render: ({ slots }) => <>{slots.children}</>,
    examples: [],
  }),
  defineEntry({
    name: "Menu",
    kind: "molecule",
    description: "A button that opens a list of actions.",
    source: "kit/ui#Menu",
    since: "1.0.0",
    props: z
      .object({
        label: name(120),
        items: z
          .array(
            z
              .object({
                id: identifier,
                label: name(120),
                disabled: z.boolean().default(false),
                tone: z.enum(["danger"]).optional(),
              })
              .strict(),
          )
          .min(1)
          .max(20),
      })
      .strict(),
    events: {
      select: {
        description: "An item was chosen.",
        payload: z.object({ id: identifier }),
      },
    },
    a11y: {
      role: "button",
      pattern: "menu button",
      name: { prop: "label" },
      keyboard: ["Enter", "Space", "Arrow keys", "Home", "End", "Escape"],
      notes:
        "Escape and outside presses close the menu and return focus to the trigger.",
    },
    surfaces: everywhere,
    render: ({ props, emit, bound }) => (
      <Menu
        label={props.label}
        items={props.items.map((item) => ({
          ...item,
          disabled: item.disabled || !bound("select"),
          onSelect: () => emit("select", { id: item.id }),
        }))}
      />
    ),
    examples: [
      {
        id: "menu",
        state: "closed",
        actions: ["row"],
        node: (t) => ({
          component: "Menu",
          props: {
            label: t("actions"),
            items: [
              { id: "duplicate", label: t("duplicate") },
              { id: "archive", label: t("archive") },
              { id: "delete", label: t("delete"), tone: "danger" },
            ],
          },
          action: "row",
        }),
      },
    ],
  }),
  defineEntry({
    name: "Dialog",
    kind: "molecule",
    description: "A modal dialog with focus containment and restoration.",
    source: "kit/ui#Dialog",
    since: "1.0.0",
    props: z
      .object({
        title: name(200),
        description: plainText(1000).optional(),
        dismissible: z.boolean().default(true),
      })
      .strict(),
    events: {
      close: {
        description:
          "Escape, the backdrop or a footer action closed the dialog.",
      },
    },
    a11y: {
      role: "dialog",
      pattern: "dialog (modal)",
      name: { prop: "title" },
      keyboard: ["Tab", "Shift+Tab", "Escape"],
      notes: "aria-modal; focus returns to the opener.",
    },
    surfaces: [],
    codeOnly:
      "A document may not open a modal over its host; hosts open Dialog from their own controls.",
    render: () => null,
    examples: [],
  }),
];
