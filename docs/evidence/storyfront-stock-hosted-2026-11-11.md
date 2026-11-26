# Current stock in the original Storyfront — 11 November 2026

The original renderer now separates current native availability from published
design. Exact tenant/channel SKU reads set its existing sold-out state. A
one-second coalesced read and stock-aware HTML cache identity preserve the
cached presentation without retaining stale purchase controls. Native variant
admission and minimum purchase quantity are included. Core still validates
stock and commits the eventual order.

The private package passed both application CI runs, including the actual
native image boot with original Astro applications and isolated PostgreSQL
databases. The Experience candidate `honest-holiday-8885`, source `643010e5`,
passed against all eight previous hosted artifacts and its unchanged storage
schema before rollout. After deployment, the same candidate matched all eight
running services and their exact source/schema inputs. The checked composition
lock now records that accepted image.

The public original Storyfront browser journey on 11 November checked stock
zero with a disabled purchase control, then added exactly one synthetic unit
through the native product editor. Reloading the same product route without
query parameters enabled purchase without a Storyfront republish. Embedded
demo order `RAC-4e0b16ed` stored the German product name and EUR 59.90 total.
Return and full reload kept the original bag empty. The native product moved
from revision 30 through restock revision 31 to purchase revision 32 with stock
zero. The same reloaded Storyfront route again disabled purchase.

Studio showed the exact order, its `order.placed` Flow entry and the manually
created German invoice `INV-00000002`. The existing Google account renewed its
Experience session and returned to Core Studio. Its earlier expired Studio
view resumed into the original order after choosing the same account.
This journey used simulated payment and an `example.test` address. It did not
establish provider capture, invoice PDF contents or actual email receipt. A page
already open refreshes availability on its next render, not through a stock push.

![Original Storyfront disables purchase for current native stock zero](../assets/checkout/storyfront-stock-disabled-20261111.jpg)

![Public embedded demo purchase after the availability rollout](../assets/checkout/storyfront-stock-purchase-20261111.jpg)

![The original bag remains empty after return and full reload](../assets/checkout/storyfront-stock-empty-20261111.jpg)

This record belongs to the earlier release that ran in the hosted pilot; see [status](../status.md).
