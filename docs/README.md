# Handbook index: one owner page per topic

Each topic below has one owner page. Other pages link to it instead of repeating it;
when behaviour changes, the owner page changes in the same commit. Dated acceptance
records live in [`evidence/`](evidence/), release entries in the
[changelog](../CHANGELOG.md). The [documentation site guide](documentation-site.md)
explains how the handbook is built and which pages are generated.

## Start

| Topic | Owner page |
| --- | --- |
| What Veyndo is and where to begin | [Start here](start-here.md) |
| Release state, what was verified, what is open | [Status](status.md) |
| What changed in this release | [Release notes](current-release.md) |
| Stages and what comes next | [Roadmap](roadmap.md) (generated from `roadmap.json`) |
| Run it locally | [Quickstart](quickstart.md) |
| One complete round: change, review, publish | [First tour](first-tour.md) |
| Every feature in one place | [Feature guide](features.md) |
| Overview in German | [Platform overview (Deutsch)](platform-overview.md) |
| Films and tours | [Video tours](video-tours.md) |

## Architecture and verification

| Topic | Owner page |
| --- | --- |
| The 22 components, ports, data, frontend, tests and proofs | [Component architecture](architecture.md) |
| How a request runs; checkout, stock, money, row security, caches, admission | [Production architecture](production-architecture.md) |
| Which path owns what; which suite checks it | [Source map](source-map.md) |
| Dependencies between components (generated) | [Component dependencies](component-dependencies.md) |
| Using one component from your own system; repositories | [One shop, composable parts](one-shop-components.md), [component repositories](commerce-components.md) |
| Standalone component mirrors | [Component mirrors](component-mirrors.md) |
| OpenAPI contracts and generated TypeScript, Python and Rust clients | [Contracts and SDKs](sdks.md) |
| The `veyndo` command-line client for merchants and developers | [CLI](cli.md) |
| Connector service, schema baselines, checked queries, Python boundary | [Rust services](rust-services.md) |
| Suites, CI cadence, coverage | [Testing](testing.md) |
| What the Lean proofs cover | [Formal verification](formal-verification.md), [proof coverage](proof-coverage.md) (generated), [domain inventory](formal-coverage.md) (generated) |
| Measured performance | [Benchmarks](benchmarks.md), [read performance](read-performance.md), [scalability](scalability.md) |
| Every source file (generated) | [Source inventory](module-inventory.md) |
| The restructuring plan and checklist | [Component architecture plan](plans/2026-11-19-component-architecture.md) |

## Agents buy

| Topic | Owner page |
| --- | --- |
| Shopping agents, cart transfer, signed offers | [Agentic commerce](agentic-commerce.md), [let an agent buy](agent-purchase.md) |
| Chat clients and MCP | [Connectors](connectors.md) |
| Storefront and Studio experience | [UI experience](ui-experience.md) |
| Checkout and the payment boundary | [Checkout](checkout.md) |
| Separately hosted Checkout, Knowledge, Automation, connectors | [Checkout component](checkout-component.md), [Knowledge component](knowledge-component.md), [Automation component](automation-component.md), [connector component](connector-component.md) |
| Hosted API only | [Hosted component API](hosted-component-api.md) |
| Agentic UI across components (MCP Apps, OpenUI, A2UI) | [Agentic component UI](agentic-component-ui.md), [Product Guide](product-guide-mcp-app.md) |
| Design system, component catalog and gallery | [Design system](design-system.md), [Component gallery](component-gallery.md) |
| Your own frontend; Storyfront | [Experience integration](experience-integration.md), [Storyfront](storyfront.md) |
| B2B negotiation over UCP | [B2B and UCP](b2b-ucp-negotiation.md) |
| External access decisions | [Connected commerce access](integrations/connected-commerce-access.md) |

## Agents work

| Topic | Owner page |
| --- | --- |
| The AI workspace and saved agent runs | [AI workspace](agentic-workspace.md), [durable agent runs](durable-agent-runs.md) |
| Agent rights, budgets and the stop switch | [Agent authority controls](agent-authority-controls.md), [agent workshop](agent-workshop.md) |
| Shop knowledge, experiments, autopilot | [Knowledge workspace](knowledge-workspace.md), [how the intelligence layer works](cognitive-commerce.md) |
| Visitor behaviour | [Behaviour intelligence](behavior-intelligence.md) |
| Testing with fictional customers; shop twin | [Customer scenarios](customer-scenarios.md), [shop twin](shop-twin.md) |
| Rules and flows; comparing a rule with history | [Rules and flows](automation.md), [rule history comparison](rule-history-comparison.md) |
| Reports, grids, canvas, workspace design | [Merchant reports](merchant-reports.md), [agentic grids](agentic-grids.md), [visual canvas](visual-workspace-canvas.md), [personal workspace](personal-workspace-and-shop-design.md) |
| Importing an existing shop | [Bring an existing shop](agentic-migration.md), [catalogue from the assistant](merchant-catalogue-setup.md) |
| Sovereign AI mode | [Sovereign AI](sovereign-ai.md) |

## Build apps

| Topic | Owner page |
| --- | --- |
| The app contract | [App platform](app-platform.md) |
| Building with Studio; workshops | [App Studio](app-studio.md), [App Studio workspace](app-studio-workspace.md), [app tutorials](app-tutorials/README.md) |
| App data, extensions and lifecycles | [App data storage](app-data-storage.md), [core extensions](app-core-extensions.md), [native lifecycles](app-native-lifecycles.md), [assistants](app-assistants.md) |
| Independent app services | [Runtime](app-runtime-service.md), [data reads](app-data-service.md), [writes](app-data-writes.md), [settings](app-settings-service.md), [installation](app-package-installation-service.md), [jobs](app-job-supervision-service.md) |
| App library and ownership | [App library](app-library.md), [plugin ownership](plugin-ownership.md) |
| App isolation | [App security](app-security.md) |
| Business capabilities and the operation registry | [Business capabilities](business-capabilities.md), [furniture configurator](furniture-configurator.md) |
| Provider apps and services | [Connected apps](connected-apps.md), [service integrations](service-integrations.md), [email delivery](email-delivery.md), [payment provider API](payment-provider-api.md) |
| Product editing API and integration keys | [Studio API and channels](studio-api-and-channels.md) |
| Where to contribute | [Early contributors](early-contributors.md) |

## Run the shop

| Topic | Owner page |
| --- | --- |
| Setup, customers and admin orders | [Shop setup](shop-setup.md), [admin workflows](admin-workflows.md) |
| Products, categories, media, 3D | [Product management](product-management.md), [settings and media](settings-media.md), [3D models](product-3d-models.md) |
| Accounts, orders, documents | [Merchant operations](merchant-operations.md) |
| Shipping | [Shipping workflows](shipping-workflows.md) |
| Payments after checkout | [Payments](payments.md), [provider acceptance](shopware-payments-acceptance.md) |
| Currencies, countries, taxes, languages | [Currencies](currencies.md), [international commerce](international-commerce.md), [localization](localization.md) |
| Company data | [Company settings](company-settings.md) |
| Sales channels, domains, previews | [Channel management](channel-management.md) |
| POS, B2B, EDI, marketplaces, reorder network | [POS](pos.md), [B2B](b2b-commerce.md), [EDI](edi.md), [marketplaces](marketplaces.md), [trade network](trade-network.md) |
| CRM and history | [Entity history](entity-history.md) |
| European privacy and consumer rights | [European operation](european-operation.md) |
| Demo catalogues | [Fashion demo](fashion-demo.md), [playground](playground.md) |

## Stay in control

| Topic | Owner page |
| --- | --- |
| What is protected | [Security](security.md) |
| Tenant isolation and database scope | [Tenant isolation](tenant-isolation.md), [authenticated database scope](authenticated-database-scope.md) |
| Merchant and customer sessions; sign-in | [Browser sessions](browser-sessions.md), [account security](account-security.md), [Studio access](studio-access-and-setup.md) |
| Studio interface and releases | [Studio interface](studio-interface.md), [workbench](workbench.md) |
| Operator console | [Platform](platform.md) |
| Deployment and hosting | [Deployment](deployment.md), [managed hosting](managed-hosting.md), [cloud releases](cloud-releases.md) |
| Licence and brand | [Licensing](licensing.md), [branding](branding.md) |
| This handbook | [Documentation site](documentation-site.md) |
