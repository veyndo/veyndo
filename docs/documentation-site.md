# The connected Veyndo handbook

The [public handbook](https://veyndo.ai/docs/) follows one story: agents buy,
work and build on the same core that serves the storefront. Its reading path is
start here → agents buy → agents work → build → run the shop → stay in control →
why you can trust it. It opens on the Veyndo website and uses the repository's
Markdown as its source.

## Find the right starting point

| Your goal | Start here |
| --- | --- |
| Understand what Veyndo does | [Start here](start-here.md), then [let an agent buy](agent-purchase.md) |
| See what is online and actually tested | [Current status](status.md): the release state, what was verified and what is open |
| Find the page that owns a topic | [Handbook index](README.md) |
| Try a complete working round | [First tour](first-tour.md) |
| Give an agent a bounded job | [Agent workshop](agent-workshop.md) |
| Create an extension | [Visual App Studio](app-studio.md) and [five illustrated workshops](app-tutorials/README.md) |
| Design a connected shop | [Experience integration](experience-integration.md), [Storyfront](storyfront.md) and [checkout](checkout.md) |
| Deploy or assess the boundaries | [Component architecture](architecture.md), [production architecture](production-architecture.md), [deployment](deployment.md) and [security](security.md) |

Every rendered article provides its original source, chapter navigation and an
article outline. Previous/next links follow the chapter sequence. Wide tables and
code scroll within the article; the chapter menu collapses on small screens.

Search covers the **complete** Markdown inventory, including module READMEs and
historical evidence. The curated chapters lead with current guides; the complete
reference retains other sources and their original dates and scope.

## One publication, two public surfaces

The existing Pages workflow builds the complete source inventory, search index,
navigation, assets and canonical metadata. `scripts/check_site.py` checks every
local link, anchor, article, search record and sitemap entry before publication.
`site/docs-navigation.json` owns the curated chapter order;
`site/docs-copy.json` owns the interface text.

The site package copies media used by the rendered pages and the screenshot
viewer. Public manifests and downloadable projects keep their established
URLs. The film guide publishes WebVTT for browser playback and copies linked
SRT subtitles into the same site package for download. The site check compares
those downloads with their source files and verifies the published links.
Older source files are kept in the repository archive.

The private website runtime serves `/docs/` on the apex from the fixed public
GitHub Pages publication. It does not copy articles into another knowledge store
or use merchant sessions. Cookies, authorization and tenant headers are never
forwarded upstream. Only public documentation paths and GET/HEAD are accepted;
recordings use streaming and bounded ranges. API, shop and Experience hosts keep
their own existing routes. GitHub Pages remains an independently readable mirror.

## Historical links and film downloads

Old GitHub Pages prefixes (`veyndo`, `vendune`, `rust-ai-commerce`, `sellm`,
`sellama`) route to the organization Pages mirror, keeping the article path,
language query and section anchor. For direct media requests and clients without
JavaScript, use the current [Veyndo handbook](https://www.veyndo.ai/docs/) or its
[Pages mirror](https://veyndo.github.io/veyndo/docs/). The site package contains
the current SRT downloads next to the WebVTT captions ([film guide](video-tours.md)).

## Diagrams

The public handbook renders Markdown Mermaid fences through the shared
`site/docs-diagrams.js` module. The unmodified Mermaid 11.15.0 browser distribution
and its MIT notice are stored in `site/vendor/`; its package integrity and bundle
SHA-256 are recorded in `mermaid.json`. Build and site checks reject a missing or
changed bundle or a disconnected loader. No CDN or network package installation is
needed to build or read diagrams, and pages without diagrams do not load Mermaid.
Strict rendering disables diagram-defined click actions and sanitizes SVG. Readers
can fit wide diagrams to the article, scroll at full size and inspect the original
source. Without JavaScript, or when one diagram fails, its source remains readable.
Diagram controls follow the handbook's EN/DE/FR/ES language selection.

## Language and evidence

Handbook navigation, search messages and chapter descriptions ship in English,
German, French and Spanish. The [German platform overview](platform-overview.md)
is written in German. Technical guides are primarily in English. The handbook
labels each article's source language in navigation and search. Set
`<!-- veyndo-language: de -->` on a German article so that its metadata and
browser language reflect the text.

[The film index](video-tours.md) lists the dated tours and their scope.


## Build and validate

From the repository root, use a virtual environment if your Python installation
requires one, then run:

```sh
python3 -m pip install -r site/requirements.txt
python3 scripts/build_site.py
python3 scripts/check_site.py
python3 scripts/testing/source_inventory.py
python3 scripts/api_catalogue.py --check
python3 scripts/structure.py
```

The structure check also needs `npm ci` in `frontend/`. The site renderer uses
the pinned Python-Markdown dependency; ordinary commerce does not need it.
Output is ignored `.site/`. Preview it with a local static web server.

The site check covers every source document, rendered page, local link, image,
heading target, unique title, discovery metadata, search inventory and sitemap.
Missing repository targets stop the build; missing rendered anchors fail the
check. Untracked, non-ignored Markdown is included for local previews before its
first commit; only committed files reach CI. Do not put private notes in the
public repository.

## Keeping the handbook current

Each topic has one owner page; the [handbook index](README.md) lists them. When a change
alters behaviour, update the owner page in the same change and link to it from other
pages instead of repeating it. Dated acceptance records belong in [`evidence/`](evidence/)
and release entries in the [changelog](../CHANGELOG.md), not in guides.

Several pages are generated and must not be edited by hand:

| Page | Generator |
| --- | --- |
| [Component dependencies](component-dependencies.md) | `python3 scripts/components.py --write` |
| [Proof coverage](proof-coverage.md) | `python3 scripts/formal/coverage_page.py` |
| [Domain proof inventory](formal-coverage.md) | `python3 scripts/formal/domain_inventory.py --write` |
| [Roadmap](roadmap.md) | `python3 scripts/roadmap.py` from `docs/roadmap.json` |
| [Source inventory](module-inventory.md) | `python3 scripts/testing/source_inventory.py --write` |

The GitHub Pages workflow builds on every pull request and `main` push and publishes
only `main` or an explicit manual run. Passing link and build checks does not prove
every prose claim; observed behaviour, implementation scope and proposed work stay
distinct. Use [the testing guide](testing.md) and [formal scope](formal-verification.md)
for exact evidence boundaries.
