# kit/ui

Accessible atoms, molecules and headless primitives. The components carry no text
of their own: every label, title and message comes from the caller, so the
localization contract stays with the screens that use them. Styles use only design
tokens from [`kit/tokens`](../tokens/README.md).

## Modules

- [`index.ts`](index.ts): The public interface; import only this file from outside `kit/`.
- [`primitives.ts`](primitives.ts): Headless focus trap, roving focus, controllable state and outside-press handling.
- [`Button.tsx`](Button.tsx): `Button` (tones, sizes, busy state) and `IconButton` (required accessible label).
- [`Feedback.tsx`](Feedback.tsx): `Badge`, `Spinner`, `Notice` (alert for danger, status otherwise), `EmptyState`, `VisuallyHidden`.
- [`Field.tsx`](Field.tsx): `Field` binds label, hint and error to its control; `TextInput`, `TextArea`, `Select`, `Checkbox`, `Switch`.
- [`Dialog.tsx`](Dialog.tsx): Modal dialog with focus containment and restoration.
- [`Tabs.tsx`](Tabs.tsx): WAI-ARIA tabs with roving focus.
- [`Menu.tsx`](Menu.tsx): WAI-ARIA menu button.
- [`Layout.tsx`](Layout.tsx): `Stack` and `Card` on the spacing scale.
- [`catalogue.ts`](catalogue.ts): The catalogue index; a unit test fails when an export is missing from it.
- [`ui.css`](ui.css): Component styles; the architecture check refuses colour literals here.

The rendered component catalogue is the Studio developer page
`admin/developer/ui-catalogue` (`#merchant` → Developer → Components).

## Verification

Run `npm test -- kit-ui` and `npm run architecture` from `frontend/`.
