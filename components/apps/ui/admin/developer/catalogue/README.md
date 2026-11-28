# admin/developer/catalogue

The Components tab of the developer workspace: the living gallery of the
[component catalog](../../../../../../frontend/src/kit/catalog/README.md).

## Modules

- [`UICatalogue.tsx`](UICatalogue.tsx): Mounts the kit gallery in the Studio language; samples switch between English, German, French and Spanish.
- [`catalogue-i18n.ts`](catalogue-i18n.ts): The tab label in English, German, French and Spanish.
- [`ui-catalogue.css`](ui-catalogue.css): Page frame only, on design tokens.

## Verification

Run `npm test -- component-catalog kit-ui` from `frontend/`.
