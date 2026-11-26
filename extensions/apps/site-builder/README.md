# Shop design app

`manifest.json` defines the canonical pages, theme overrides and managed actions.
The Rust side is the `veyndo-site-builder` crate (`Cargo.toml`, library root
`server.rs`): `server.rs` validates the bounded document, `design_agent.rs` and
`design_agent/` prepare ordinary revision-bound native tasks with the centrally
configured inference service. Server-owned steps (catalogue page, proposal
application, surface registry, reference capture, coding-agent connection) are
reached through the `SiteHost` port in `ports.rs`; reference capture takes bounded
public HTTPS evidence or supplied images, and partial capture is explicit. No
browser source executes.

`Renderer.tsx` and `site.css` render both Studio previews and published pages.
`studio/` owns graphical controls, Tiptap text, reference alternatives and history.
`storefront/` mounts the app without replacing cart, account or checkout owners.
Global home appearance is inherited; channel home records override it. Public
queries enforce published status, time and channel on the server. Drafts stay
private. Existing staging diff/release publishes selected app packages and records.
`usePageProducts.ts` resolves explicit selections outside the first catalogue page
in native bounded batches; it does not price products or retain a separate catalogue.

Forms use the host's public form-block contract. The forms app owns intake and its
private inbox. The original private Storyfront editor remains a different app.

The Studio entry is preview-first: the existing native home is visible before a page record exists, and saved home pages open automatically. `studio/StorefrontCanvas.tsx` owns the static isolated viewport; native header/hero/product/footer **presentation** components are shared with the actual storefront. The preview creates no cart, shopper controller, tracking or payment session. `studio/home-draft.ts` converts an explicit selection to an unsaved native app document while preserving real language text and owned product-media references. `tests/` is registered through thin host test adapters; test code cannot be imported into production modules.

`studio/BlockPalette.tsx` renders illustrated native/app element cards. App elements
store references, resolve through the public app registry and use read-only preview
grants; `elements.rs` validates their tenant, active package and public native surface.

Storefront-kit parts (`kind: "part"`, currently the product card and the buy box)
are placed from the same palette. `site-parts.tsx` lists the placeable kit
contracts, resolves their product from the page products and renders the kit
component; the inspector form is the kit `PartForm` generated from the part's
configuration schema. `part-schemas.json` is generated from those contracts by
`frontend/tests/unit/site-parts.test.tsx` (`WRITE_PART_SCHEMAS=1`), which fails on
drift; `part_schemas.rs` validates stored configurations against it on every save,
and product IDs it names pass the same tenant ownership check as product blocks.
The agent planning contract embeds the same schemas.
`proposals.rs` owns shared proposal validation and persistence; `agent_handoff.rs`
exports credential-free tasks and validates coding-agent results through that owner.
Shared agent controls stay in the generic host; page business behavior stays here.

Ordinary pages and design assistance require only the site-builder package. The
optional form receiver is reviewed separately when the selected page contains a
form. Existing package consent, sandbox and channel-editing rights still apply.

The app-owned immutable `prompts/site-design.v1.txt` and manifest register
`site-design@1` through the existing inference evidence. Style-only proposals
preserve native content directly; the provider returns `{}` for the unused document.
Section selection defaults to content editing; page-wide theme controls explain
the Whole page scope. Unsupported direct section-only theme requests reject before
inference. Section selection defaults the contextual agent target while an explicit whole-page
choice remains stable until selection changes. Real browser/reference acceptance:
`docs/evidence/designer-2026-10-29.md`.
