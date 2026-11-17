# Your first tour: change, review, publish

In about ten minutes you run a shop on your own machine, place an order, change
a product in private and publish only that change. No model, API key or
veyndo.ai account is needed. All data is synthetic and payments are simulated.

Prefer to watch first? The [1:56 overview film](assets/showcase/product-films-20261102/veyndo-ai-commerce.mp4)
shows the agent workspace, visual design and app platform, with
[English captions](assets/showcase/product-films-20261102/veyndo-ai-commerce.en.vtt)
and no sound. [All product films](video-tours.md).

[![Veyndo Commerce product workspace](assets/showcase/product-films-20261102/veyndo-workspace.jpg)](video-tours.md)

## 1. Start the shop

You need Git, Docker with Compose and OpenSSL. You do not need Rust, Node or
Python.

```sh
git clone --depth 1 https://github.com/veyndo/veyndo.git
cd veyndo
./scripts/try.sh
```

Open **http://127.0.0.1:8788/**. The first start pulls the image and can take a
while.

## 2. Create your shop

Choose **Studio → Create shop**. Create a personal owner account and a
synthetic shop. Both exist only on your machine, in the Compose project's named
volume.

## 3. Place an order

In the storefront, choose a coat size and place one **simulated** order. Then
open Studio and find that order. You have just used the same checkout that
agents and the SDK use.

## 4. Change a product in private

1. In Studio's environment controls, create a **sandbox** from your shop and
   switch into it. Check the displayed environment before you edit.
2. Open a product and change its description to a clearly fictional tour note.
3. Save. The live product still shows its original description.

## 5. Review and publish one change

1. Open **Review changes**.
2. Select only that product change and compare it with the live baseline.
3. Publish the selected change.
4. Return to the live product and check the description.

Publishing content keeps the live stock. If someone edits the live product at
the same time, you get a conflict to resolve. Nothing is overwritten silently.

That is the full round: a real order, a private change, a selected release.

## Go further

| Next | What you do | Needs a model |
| --- | --- | --- |
| [Let an agent buy](agent-purchase.md) | Search, quote and order over MCP with `curl` | No |
| [Set up your shop](shop-setup.md) | Six review areas in Studio with a saved return point | No |
| [Ask the agent for a proposal](connectors.md) | Request a bounded product change and inspect its fields before approval | Yes |
| [Test with fictional customers](customer-scenarios.md) | Give customers a goal in the sandbox, change one setting, run them again and compare | Yes |
| [Shop by intent](ui-experience.md#standard-storefront-and-customer-account) | Ask for a gift or comparison as a shopper and edit the selection | Yes |
| [Build an app](app-studio.md) | Describe an app with its own data and views | No |

A configured model or a generated summary does not prove that an action
happened. Always check the saved record.

## Stop and come back

```sh
./scripts/try.sh stop
./scripts/try.sh
```

Stopping keeps your orders and accounts. Do not remove the data volume if you
want to keep them.

## Options and limits

- **Another port or project.** Run a second tour without touching other local
  shops, and reuse the same values when you restart:

  ```sh
  TRY_PORT=8789 TRY_PROJECT=veyndo-my-tour ./scripts/try.sh
  ```

- **ARM machines.** The published image targets **Linux amd64**. ARM hosts need
  Docker's x86 emulation. Native ARM packaging and its acceptance are not claimed.
- **No image available.** [Build from source](quickstart.md).
- **An exact version.** The preview tag moves. Set
  `VEYNDO_IMAGE=ghcr.io/veyndo/veyndo:sha-COMMIT_SHA` with a full commit SHA
  from a successful **Publish tested local preview image** run. Documentation-only
  commits do not create another image. A manual **Run workflow** on main can
  rebuild intentionally; a documentation commit landing during a build does not
  prevent the tested runtime from advancing the preview tag.

## Tell us what happened

[Tell us where you got stuck or what you built](https://github.com/veyndo/veyndo/discussions)
or [report a reproducible bug](https://github.com/veyndo/veyndo/issues/new/choose).
Include your source or image version, operating system and the first unexpected
step. Keep credentials and customer data out of public reports.

[Sandbox boundaries](workbench.md) · [Connect an agent](agent-workshop.md) ·
[First contributions](early-contributors.md)
