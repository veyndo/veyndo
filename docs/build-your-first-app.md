# Build your first app in 10 minutes

From nothing to an app running in your own private sandbox, visible in Studio.
You need a terminal, Node.js 22.18 or newer, and either a hosted shop or a
local Veyndo. Nothing here touches live orders, customers or payments.

| Step | What you do | Time |
| --- | --- | --- |
| [1. Account](#1-create-an-account-and-a-shop) | Create a shop and a personal merchant account | 2 min |
| [2. Session](#2-get-a-personal-session) | Get a session token and your shop ID | 1 min |
| [3. Template](#3-pick-a-template) | Copy one of four templates and run its tests | 2 min |
| [4. Deploy](#deploy-to-a-private-sandbox) | Install the app in a new private sandbox | 1 min |
| [5. Studio](#5-see-it-in-studio) | Open the sandbox and use the app | 3 min |

## 1. Create an account and a shop

**Hosted:** open [Veyndo Start](https://start.veyndo.ai/start) and create a shop.
Your personal merchant account owns it. The demo catalog (Nord Atelier, see the
[fashion demo](fashion-demo.md)) is optional; the templates work on any catalog.

**Local:** follow the [local quickstart](quickstart.md), open
`http://127.0.0.1:8787/#merchant` and choose **Create shop**. Use
`VEYNDO_URL=http://127.0.0.1:8787` in the steps below.

There is no shared public demo shop. Every example in this handbook runs
against a shop you own.

## 2. Get a personal session

A personal session is your own merchant sign-in as a bearer token. Deploying
an app needs it; an integration key cannot install apps.

```sh
export VEYNDO_URL=https://api.veyndo.ai
read -r VEYNDO_EMAIL
read -rs VEYNDO_PASSWORD
printf '{"email":"%s","password":"%s"}' "$VEYNDO_EMAIL" "$VEYNDO_PASSWORD" |
  curl -fsS "$VEYNDO_URL/api/auth/login" -H 'content-type: application/json' --data @- > session.json
export VEYNDO_TOKEN=$(node -p 'require("./session.json").token')
export VEYNDO_SHOP=$(node -p 'require("./session.json").workspace')
rm session.json; unset VEYNDO_PASSWORD
```

The password goes through standard input, not the command line. The session
expires after 12 hours; sign in again when a call returns
`authentication_required`. Keep the token out of files you commit.

Check the shop with a public read. It needs no token:

```sh
curl -fsS -X POST "$VEYNDO_URL/store-api/product" -H "x-tenant: $VEYNDO_SHOP" \
  -H 'content-type: application/json' --data '{"limit":3}'
```

Or paste your shop ID into any **Try it** panel of the
[API reference](reference/README.md#try-it-from-this-page).

## 3. Pick a template

| Template | You get | Where it shows |
| --- | --- | --- |
| [`product-badge`](../templates/product-badge/README.md) | A badge such as New or Sale, a product tab to set it and a settings page | Product editor, product page |
| [`order-tags`](../templates/order-tags/README.md) | Order tags and a flow that suggests a VIP tag for large orders | Order editor, Flows |
| [`shopper-guide`](../templates/shopper-guide/README.md) | A product guide as an MCP App for shopping assistants, with a private merchant view | Storefront assistant, Studio assistant |
| [`storefront-starter`](../templates/storefront-starter/README.md) | A headless storefront on the TypeScript SDK, in four languages | Your own Node server |

Copy the folder from a checkout of this repository:

```sh
git clone --depth 1 https://github.com/veyndo/veyndo.git
cp -R veyndo/templates/product-badge my-badge
```

The [`veyndo` CLI](https://www.veyndo.ai/docs/cli.html) offers a second set of starting
points, the App Studio patterns, and deploys them through the native package review
and install path: `veyndo app new advisor|returns|erp|edi|furniture <dir>`, then
`veyndo app deploy`. It does not scaffold the four templates above yet.

Then run the tests. They need no network and no account:

```sh
cd my-badge
npm test
```

Every app template is a `manifest.json` (the app's data, views, actions and
permissions), tests and `scripts/deploy-sandbox.mjs`. Give the app its own ID
first: change `"id"` in `manifest.json` to a new
snake_case name. The [App platform](app-platform.md) describes every field.

## Deploy to a private sandbox

```sh
npm run deploy
```

The script makes three calls with your session, the same ones App Studio's builder uses:

1. `POST /api/environments` creates a private sandbox, a copy of your shop
   that only you can see.
2. `POST /api/developer/import` saves `manifest.json` as an immutable build in
   that sandbox. The server compiles it here; its errors name the field.
3. `POST /api/developer/builds/{id}/stage` installs exactly that build digest.

Templates with a `flows/` folder also save each flow in the sandbox. Set
`VEYNDO_SANDBOX=<sandbox id>` to reuse a sandbox; raise `"version"` in the
Manifest for every new build. `npm run deploy:dry-run` lists the calls without
sending them.

```text
Sandbox: stage-065ea58240864983ab1b
Build: 821010e0ce9c4cb8b17642b6b18657f3 (122e8d0d…)
Installed my_badge 0.1.0 in sandbox stage-065ea58240864983ab1b.
```

A sandbox has its own records and test data. Schedules and external delivery
are off there, and anonymous Store API reads are refused, so a storefront or a
Try-it panel cannot read it. Releasing to the live shop is a separate, reviewed
step: [staging and releases](app-tutorials/README.md#the-same-release-steps-for-every-workshop).

## 5. See it in Studio

1. Open Studio (`$VEYNDO_URL/#merchant` locally, or your Studio link from
   Veyndo Start) and sign in.
2. Choose the sandbox (named after the app, for example **Product badge
   sandbox**) in the environment menu of the top bar, which shows **Live shop**
   until you switch. A banner marks the private sandbox.
3. The app's own pages appear in the navigation, for example **Badge
   settings** with the app ID and version in its header.
4. Use it where the template shows it: open a product and its **Badge** tab,
   an order and its **Tags** tab and **Rules & flows**, or the assistant for
   the shopper guide.

The storefront starter is not installed in Studio; run it with `npm start`
against your shop ID and open `http://localhost:3000`.

## Next

- Change the template, raise its version and deploy again.
- Read every call your app can make in the [API reference](reference/README.md).
- Generate clients from the same contracts: [SDKs](sdks.md).
- Connect your coding agent to the shop: `veyndo mcp --client claude-code` (or
  `claude-desktop`, `cursor`) prints an MCP configuration without secrets.
- Build the five longer [app workshops](app-tutorials/README.md), or let a
  coding agent extend your Manifest through [App Studio](app-studio.md).

The previous connected playground page now redirects here; its legacy
furniture walkthrough stays available as [the legacy playground](playground.md).
