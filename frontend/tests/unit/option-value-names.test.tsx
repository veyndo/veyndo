/** Variant option value names: one content language in the editor, main-language inheritance, no copied translations, and storefront labels from the server. */
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { expect, it } from "vitest";
import OptionValueNames, {
  familyValues,
  withName,
} from "../../../components/catalog/ui/admin/catalog/OptionValueNames";
import type { ProductDraft } from "../../../components/catalog/ui/admin/catalog/catalog-model";
import { ContentLanguage } from "../../src/shared/i18n/ContentLanguage";
import { LocaleProvider } from "../../src/shared/i18n/i18n";
import {
  optionLabel,
  optionText,
} from "../../src/shared/content/option-labels";

const locales = ["en-GB", "de-DE", "es-ES", "fr-FR", "it-IT"];
function draft(optionValues?: unknown): ProductDraft {
  return {
    id: "coat",
    revision: 1,
    mainLocale: "en-GB",
    availableLocales: locales,
    translations: {},
    extra: optionValues ? { optionValues } : {},
    commerce: {},
    catalog: {
      active: true,
      productNumber: "coat",
      categoryIds: [],
      parentId: null,
      options: { color: "camel", size: "M" },
    },
  };
}
function Editor({
  language,
  start,
  onDraft,
}: {
  language: string;
  start: ProductDraft;
  onDraft: (d: ProductDraft) => void;
}) {
  const [value, setValue] = useState(start);
  return (
    <ContentLanguage locales={locales} mainLocale="en-GB" language={language}>
      <OptionValueNames
        draft={value}
        variants={[{ options: { color: "camel", size: "L" } }]}
        onChange={(d) => {
          setValue(d);
          onDraft(d);
        }}
      />
    </ContentLanguage>
  );
}

it("collects each value of the family once, in first-seen order", () => {
  expect(
    familyValues([
      { color: "camel", size: "M" },
      { color: "camel", size: "L" },
    ]),
  ).toEqual([
    ["color", ["camel"]],
    ["size", ["M", "L"]],
  ]);
});

it("saves only written names; clearing a name restores inheritance", () => {
  const names = withName({}, "color", "camel", { de: "Kamel", es: "" });
  expect(names).toEqual({ color: { camel: { de: "Kamel" } } });
  expect(withName(names, "color", "camel", { de: "" })).toEqual({});
});

it("edits a fifth content language and shows the main-language name as its fallback", async () => {
  const user = userEvent.setup();
  let saved: ProductDraft | undefined;
  render(
    <Editor
      language="it-IT"
      start={draft({ color: { camel: { en: "Camel" } } })}
      onDraft={(d) => (saved = d)}
    />,
    { wrapper: LocaleProvider },
  );
  const camel = screen.getByLabelText("camel");
  expect(camel).toHaveValue("");
  expect(camel).toHaveAttribute("placeholder", "Camel");
  // An unnamed value falls back to the value itself, in every language.
  expect(screen.getByLabelText("L")).toHaveAttribute("placeholder", "L");
  await user.type(camel, "Cammello");
  expect(saved?.extra.optionValues).toEqual({
    color: { camel: { en: "Camel", "it-IT": "Cammello" } },
  });
});

it("reads storefront names resolved by the server and falls back to the value key", () => {
  const family = [
    {
      options: { color: "camel", size: "L" },
      extra: { optionLabels: { color: "Kamel", size: "L" } },
    },
    { options: { color: "warm-gray", size: "L" } },
  ];
  expect(optionLabel(family, "color", "camel")).toBe("Kamel");
  expect(optionLabel(family, "color", "warm-gray")).toBe("warm-gray");
  expect(optionText(family[0])).toBe("Kamel / L");
});
