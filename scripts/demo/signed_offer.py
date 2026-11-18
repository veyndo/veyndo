#!/usr/bin/env python3
"""Buying-agent view of a signed offer: fetch it for a cart, verify the Ed25519 signature offline, then ask the shop whether checkout would confirm it.

Usage: python3 scripts/demo/signed_offer.py [BASE_URL] [SHOP] [PRODUCT_ID]
Needs the `cryptography` package. Places no order and changes no price.
"""
import base64, json, sys, urllib.request, uuid
from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey

base = sys.argv[1] if len(sys.argv) > 1 else 'http://127.0.0.1:8787'
shop = sys.argv[2] if len(sys.argv) > 2 else 'atelier'
product = sys.argv[3] if len(sys.argv) > 3 else 'lamp'


def call(path, body=None, headers=None):
    request = urllib.request.Request(base + path, data=None if body is None else json.dumps(body).encode(), headers={'Content-Type': 'application/json', 'x-tenant': shop, **(headers or {})})
    with urllib.request.urlopen(request, timeout=30) as response:
        return json.load(response)


def decode(value):
    return base64.urlsafe_b64decode(value + '=' * (-len(value) % 4))


def authentic(key, payload, signature):
    try:
        key.verify(decode(signature), decode(payload))
        return True
    except InvalidSignature:
        return False


cart = call('/store-api/checkout/cart', {'session': 'agent-' + uuid.uuid4().hex})
mine = {'sw-context-token': cart['token']}
call('/store-api/checkout/cart/line-item', {'items': [{'referencedId': product, 'quantity': 2}]}, mine)
offer = call('/store-api/checkout/cart/offer', headers=mine)
signed = offer['payload']
print(f"1. Offer: {signed['lineItems'][0]['quantity']} x {signed['lineItems'][0]['label']}, total {int(signed['totalMinor']) / 10 ** signed['currencyScale']:.{signed['currencyScale']}f} {signed['currency']}, cart revision {signed['cartRevision']}, valid {signed['expiresAt'] - signed['issuedAt']} s")
jwk = call(offer['verificationKeys'])['keys'][0]
key = Ed25519PublicKey.from_public_bytes(decode(jwk['x']))
print(f"2. Offline check with key {jwk['kid'][:16]}: {'signature valid' if authentic(key, offer['payloadBase64'], offer['signature']) else 'SIGNATURE INVALID'}")
forged = base64.urlsafe_b64encode(json.dumps({**signed, 'totalMinor': '1'}).encode()).decode().rstrip('=')
print(f"3. Same offer with the total changed to 0.01: {'signature valid' if authentic(key, forged, offer['signature']) else 'rejected offline'}")
answer = call('/store-api/checkout/offer/verify', {'payloadBase64': offer['payloadBase64'], 'signature': offer['signature']}, mine)
print(f"4. Shop says: valid={answer['valid']}, expired={answer['expired']}, checkout would confirm it={answer['honoured']}")
call('/store-api/checkout/cart/line-item', {'items': [{'referencedId': product, 'quantity': 3}]}, mine)
answer = call('/store-api/checkout/offer/verify', {'payloadBase64': offer['payloadBase64'], 'signature': offer['signature']}, mine)
print(f"5. After the cart changed: still authentic={answer['valid']}, checkout would confirm it={answer['honoured']}")
