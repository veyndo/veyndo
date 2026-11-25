/** The component catalogue's index: every public kit/ui component with its kind and the WAI-ARIA pattern it follows. */
export type CatalogueEntry = {
  name: string;
  kind: "atom" | "molecule" | "primitive";
  pattern?: string;
};
export const catalogue: CatalogueEntry[] = [
  { name: "Button", kind: "atom", pattern: "button" },
  { name: "IconButton", kind: "atom", pattern: "button" },
  { name: "Badge", kind: "atom" },
  { name: "Spinner", kind: "atom", pattern: "status" },
  { name: "VisuallyHidden", kind: "atom" },
  { name: "TextInput", kind: "atom" },
  { name: "TextArea", kind: "atom" },
  { name: "Select", kind: "atom" },
  { name: "Checkbox", kind: "atom", pattern: "checkbox" },
  { name: "Switch", kind: "atom", pattern: "switch" },
  { name: "Field", kind: "molecule" },
  { name: "Notice", kind: "molecule", pattern: "alert / status" },
  { name: "EmptyState", kind: "molecule" },
  { name: "Card", kind: "molecule" },
  { name: "Stack", kind: "atom" },
  { name: "Dialog", kind: "molecule", pattern: "dialog (modal)" },
  { name: "Tabs", kind: "molecule", pattern: "tabs" },
  { name: "Menu", kind: "molecule", pattern: "menu button" },
  { name: "useField", kind: "primitive" },
  { name: "useFocusTrap", kind: "primitive" },
  { name: "useControllable", kind: "primitive" },
  { name: "useOutsidePress", kind: "primitive" },
  { name: "rovingIndex", kind: "primitive" },
  { name: "focusables", kind: "primitive" },
];
