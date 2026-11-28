# Site builder UI regressions

The shared frontend runner registers these app-owned cases through a thin host adapter. The host replaces the browser viewport for component tests; real browser acceptance separately verifies the isolated native preview and editing. No product implementation or app fixtures are copied into the Core.

`interactions-cases.tsx` covers palette insertion/order, opaque drag tokens, stale channel changes, full-page caps, removal, native-home first-insertion undo and read-only callers. `design-reference-cases.tsx` covers local screenshot reduction, dimension rejection, resource cleanup and invalid-upload recovery. `workspace-cases.tsx` exercises planner URL payloads, opt-in content copying and late proposal rejection. These are component/contract checks; they do not prove a model’s visual quality.

`brand-cases.tsx` covers the brand theme: the real storefront frame at each motion intensity with the brand tokens, company logo → derived AA theme → side-by-side comparison → draft-only apply, refused uploads, and home-only brand/motion controls. `brand-theme-fixture.json` holds derived themes written by `frontend/tests/unit/brand-theme.test.ts` (`WRITE_BRAND_FIXTURE=1`); the Rust validator and the `visual_workspace` release check read the same documents.
