# Signed offers for buying agents: local record, 18 November 2026

**Scope:** local source build on a development machine, PostgreSQL 17 in Docker,
synthetic Nord Atelier demo catalog, a locally generated signing seed. Nothing
here is deployed to the hosted pilot.

## What was observed

`python3 scripts/demo/signed_offer.py http://127.0.0.1:8787 roadmap-lab scarf`
against the running local server printed:

```text
1. Offer: 2 x Soft Wool Scarf · warm-gray / one-size, total 118.00 EUR, cart revision 2, valid 600 s
2. Offline check with key 285180ddbddd7eaa: signature valid
3. Same offer with the total changed to 0.01: rejected offline
4. Shop says: valid=True, expired=False, checkout would confirm it=True
5. After the cart changed: still authentic=True, checkout would confirm it=False
```

Step 2 and 3 use the Python `cryptography` library and the public key from
`/.well-known/commerce-facts.json`; the shop is not asked. Studio showed the
**Signed offers** card as active with the same key identifier.

![Studio card](../assets/roadmap/signed-offers-studio.jpg)

## Automated checks

- `python3 scripts/verify_integration.py --only users providers`, case "Signed
  cart offers": independent Ed25519 verification; signed payload equals the
  decoded bytes; total, revision, currency, channel and line match the native
  quote; no cart token, buyer email or stock in it; ten minutes validity; an
  empty cart has no offer (409) and a caller without cart context gets 401;
  the verify operation honours the offer for the holder of the cart, answers
  "unknown" without the cart, and refuses a forged total, another shop and
  malformed input; the MCP tools `cart.offer` and `offer.verify` agree; after a
  merchant price change and after a cart change the old offer is authentic but
  not honoured.
- Rust unit test: only the closed projection is signed; an empty cart is refused.
- Frontend: unit test for the Studio card (active and missing key), plus type
  check, localization, architecture and format checks.

## Known failure outside this change

The later personal-account case of the `providers` suite fails with 403, as on
the commits before this work.

## Not covered

- Hosted deployment. The full daily suite was not run.
- **No price lock and no stock reservation.** The offer is a verifiable
  statement; checkout keeps its own revision and total check.
- UCP carts: the offer is issued for Store API and MCP carts identified by the
  cart token. A UCP route was not added.
- AP2 payment mandates and key rotation with overlapping keys.
- Completing an order after verifying the offer was not part of the demo script;
  the existing checkout already requires the reviewed revision and total.
- French and Spanish texts were written, not reviewed by native speakers.
