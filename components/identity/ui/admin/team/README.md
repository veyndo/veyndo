# admin/team

Personal accounts, memberships, roles, invitations and scoped developer access.

Files and their individual responsibilities are listed in [the generated source inventory](../../../../../docs/module-inventory.md). Each file starts with its contract summary.

## Modules

- [`AccessManager.tsx`](AccessManager.tsx): Fine-grained team overrides, revocable invitations and personal session inventory.
- [`IntegrationKeys.tsx`](IntegrationKeys.tsx): Integration keys (table `integration_keys`): scoped rights, lifetime, one-time secret display and revocation.
- [`AgentBudgetFields.tsx`](AgentBudgetFields.tsx): A key's agent authority: model provider, model and allocated budget.
- [`agent-control-i18n.ts`](agent-control-i18n.ts), [`integration-keys-i18n.ts`](integration-keys-i18n.ts), [`integration-keys.css`](integration-keys.css): Their vocabulary and styles, also used by the developer agent controls.
- [`PersonalAccountForm.tsx`](PersonalAccountForm.tsx): PersonalAccountForm: focused account-form view with explicit typed inputs and callbacks.
- [`UsersManager.tsx`](UsersManager.tsx): Users Manager: Personal accounts, memberships, roles, invitations and scoped developer access..

## Studio registration

[`studio-extension.ts`](studio-extension.ts) declares this area's Studio routes and slot contributions for [the registry](../../../../../frontend/src/admin/shell/registry/README.md); [`TeamRoute.tsx`](TeamRoute.tsx) adapt the Studio context to the area's views.

## Styles

Handwritten area styles moved from `admin/styles` (unchanged rules; rebuilding on `kit/ui` follows).

- [`commerce-manager.css`](commerce-manager.css): commerce manager: Studio visual system and merchant operational layouts.
- [`team-workspace.css`](team-workspace.css): Account, shop creation and team access share a clear action and form hierarchy.
- [`team-roles.css`](team-roles.css): Native team roles and readable scopes share Studio surfaces and responsive form rhythm.

## Verification

Run `npm run build`, `npm test`, `npm run test:coverage` and `npm run architecture` from `frontend/`. Coverage includes untested source files; a module in this folder is not automatically fully tested. See the root [testing guide](../../../../../docs/testing.md) for backend integration and coverage limits.

`PermissionPicker` groups the canonical supplied scopes by their task area, with readable labels, technical IDs, current selection counts and search. `permission-groups` and `permission-picker.css` are shared with integration-key management; neither changes the authorization registry or grants.

`CreateWorkspace` owns only the inline creation form and editable identifier
suggestion. `UsersManager` submits through the native personal account API and
retains the selected shop on rejection. Invitations use controlled current role
options, preserving Viewer as the default through asynchronous loading.
`AccessManager` and the developer workspace share this folder's `IntegrationKeys` (one key form).
The key manager imports its own styles, so direct Team entry is fully styled.
