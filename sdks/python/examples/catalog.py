"""Example: list a shop's products page by page, then call a typed MCP tool.

Run with `VEYNDO_URL=... VEYNDO_SHOP=... python3 examples/catalog.py` from sdks/python.
"""
import os
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from veyndo_sdk import VeyndoClient, VeyndoError, paginate  # noqa: E402
from veyndo_sdk.tools import CatalogSearchInput  # noqa: E402

client = VeyndoClient(os.environ.get('VEYNDO_URL', 'https://api.veyndo.ai'),
                      tenant=os.environ.get('VEYNDO_SHOP', 'lumen-story-lab-15-o-98aa4751fa'))
try:
    pages = paginate(lambda after: client.catalog.post_store_product(body={'limit': 20, **({'after': after} if after else {})}))
    for product in pages:
        print(product['id'])
    print(client.mcp.call(CatalogSearchInput(query='lamp', limit=3)))
except VeyndoError as error:
    print(error.status, error.kind, error, file=sys.stderr)
