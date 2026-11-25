# App Studio

A visual workbench over the **same versioned Manifest** consumed by the Rust runtime, HTTP APIs and coding-agent imports. The browser compiles data bindings into existing authorized actions; the Rust server validates every installed package independently.

- `DeveloperView.tsx`: workspace, tab navigation, one content-language scope and sandbox selection.
- `app-model.ts`: pure schema edits, action binding, dependency cleanup and bounded undo/redo.
- `AppCanvas.tsx`: accessible click-to-add/select/order/remove components.
- `AppInspector.tsx`: app/view/block properties with inherited translations.
- `AppDataEditor.tsx`: typed managed data fields and public-read settings.
- `AppConnections.tsx`: HTTP routes, AI grounding/tools and Flow Builder opt-in.
- `AppAgentPanel.tsx`: current-manifest prompting, Codex/Claude task export, authoritative schema download and agent JSON round-trip.
- `useAppStudio.ts`: immutable build lifecycle and exact saved-snapshot checks.
- `AppVersions.tsx`: version inspection, digest-approved staging and selective package release.
- `SandboxPreview.tsx`: authoritative installed-registry resolution, including rejection of a stale preview version.

The actual renderer is in `shared/apps/native/`, shared by the sandbox preview and released admin/storefront views. Freely executable app services retain the isolated iframe/service path; the native builder does not execute scripts or start a shell.

Run frontend build, tests, coverage, localization and architecture checks. See [App Studio guide](../../../../docs/app-studio.md) and [testing boundaries](../../../../docs/testing.md). Test presence is not 100% whole-codebase coverage.

- `AppLibrary.tsx`: clickable saved app cards with explicit edit/delete, confirmation and restore; opens the latest build as a new editable version. Both library and version-list editing return to the design workspace.

- `AppAssistant.tsx`, `assistant-model.ts`: nine guided types compiled into the shared versioned contract.
- `AppFieldOptions.tsx`, `AppContextBinding.tsx`: owned core references, translated choices and host context.
- `AppActionAccess.tsx`: independent team scopes and MCP flags.
- `AppAutomation.tsx`: declared service events, UTC cron and signed ingress.
- `AppViewTabs.tsx`: view navigation and creation.
- `SandboxContextPicker.tsx`: bounded selection of real sandbox products/customers/orders.

[Guided apps, provider boundaries and verification](../../../../docs/app-assistants.md).

## Multi-placement workbench

`assistant-placements.ts` compiles up to six selected areas into ordinary views/models in one package, retaining context-bound product/customer/order editors. `AppDesignWorkspace` composes the native preview and inspector; `AppPanelSizing` stores bounded presentation widths only. `AppControlThumbnail` provides passive illustrations. Stack/grid drag/drop and 12-column layout use the same Manifest blocks. `AppFormFields` edits the real selected model captions, not fake sample records.

`AppSettingsEditor` edits setting declarations only. Installed values use `admin/apps/AppSettings` and the existing encrypted app-secret owner. Settings do not enter templates, agent task exports or public native payloads. Their revisions/audit metadata describe credential rotation; they are live-only and are not configuration rollback/release units.

`AppLogicTopology` projects the existing bounded UI AST into the shared Flow Builder topology. It never persists or executes a second flow. `AppFlowNavigation` opens the native Flow Builder; native app saves already produce its `app.record.changed` event. `AppAgentDrawer` opens the shared `AppAgentPanel` on demand from every editing section. The native modal handles focus/Escape and keeps an unfinished prompt mounted while closed. The canvas stays in place; full task/import/provider controls use the same Manifest. The toolbox uses small illustrated rows (default 180 px; adjustable 148–280 px).

[Illustrated workbench and exact runtime/security boundaries](../../../../docs/app-studio-workspace.md).

`AppBrief`, `AppDraftEntry` and `start-described-app` keep description-led creation, replacement confirmation and first-stage preparation in the normal draft/agent lifecycle. Empty private app briefs compile through the Rust owner `generation_brief`; advanced existing-app edits retain their original route. The exact immutable asset SDK JSON is shared with the native compiler; other SDK implementation imports remain outside the frontend ownership boundary.

- [AppUpgradeCheck.tsx](AppUpgradeCheck.tsx): Immutable build admission in its owned stage with account/environment response fencing.

## Studio registration

[`studio-extension.ts`](studio-extension.ts) declares this area's Studio routes and slot contributions for [the registry](../shell/registry/README.md); [`DeveloperRoute.tsx`](DeveloperRoute.tsx) adapt the Studio context to the area's views.

## Styles

Handwritten area styles moved from `admin/styles` (unchanged rules; rebuilding on `kit/ui` follows).

- [`app-creation.css`](app-creation.css): Grouped illustrated placements, compact language context and a responsive private-draft summary.
- [`app-studio.css`](app-studio.css): App Studio: restrained light canvas, compact control density and responsive palette/inspector workspaces.
- [`app-studio-panels.css`](app-studio-panels.css): App Studio model, connection, agent and version panels.
- [`app-capability.css`](app-capability.css): Merchant-friendly idea brief, explicit next steps and optional bounded rule checks share Studio tokens.
- [`app-canvas-workspace.css`](app-canvas-workspace.css): Scene overview and direct editing share the Studio canvas; tools never compete with the preview.
- [`app-workspace.css`](app-workspace.css): App Studio panel sizing, visible insertions and shared automation presentation.
- [`app-studio-preview.css`](app-studio-preview.css): Passive schema previews and semantic navigation. No alternative app runtime.
- [`app-assistant.css`](app-assistant.css): Guided extension workspace: restrained colour, clear choices and responsive setup.
- [`app-studio-inspector.css`](app-studio-inspector.css): App Studio binding indicators, empty canvas and properties inspector.
- [`app-studio-responsive.css`](app-studio-responsive.css): Responsive App Studio layouts and reduced-motion settings.
- [`app-agent-drawer.css`](app-agent-drawer.css): Contextual AI dialog presentation; native dialog owns modal focus and the editing canvas remains mounted.
- [`app-building.css`](app-building.css): Modern recipe cards and structural AI review reuse Studio colors and responsive interaction rules.
- [`app-canvas-responsive.css`](app-canvas-responsive.css): Shared app canvas keeps its native controls reachable at narrow and short viewport sizes.
- [`app-brief.css`](app-brief.css): Description-led entry keeps one clear request above the optional visual app patterns.
- [`api-console.css`](api-console.css): Responsive developer key management and bounded API explorer using Studio theme tokens.
