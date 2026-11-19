"""Keep exported host inputs connected to native deserialization fields and admission schemas."""
import json,re
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
contract=json.loads((ROOT/'extensions/sdk/host-exchange-v1.json').read_text())
assert contract['version']==1
schemas=contract['operations']
assert set(schemas)=={'record_action','channel','channel_product','order_import','job_enqueue','job_claim','job_progress'}
assert 'pub(super) use veyndo_pricing::external_order::PaidOrder as Input;' in (ROOT/'src/commerce/external_order_input.rs').read_text()
text=(ROOT/'components/pricing/src/domain/external_order.rs').read_text()
def fields(name):
    body=re.search(r'struct '+name+r' \{(.*?)\n}',text,re.S)[1]
    return {re.sub(r'_([a-z])',lambda m:m[1].upper(),f) for f in re.findall(r'pub (\w+):',body)}
order=schemas['order_import']['inputSchema']
assert set(order['properties'])==fields('PaidOrder')
assert set(order['properties']['lines']['items']['properties'])==fields('OrderLine')
assert set(order['properties']['shipping']['properties'])==fields('SourceAmount')
# The server takes Address from the shop settings, which take it unchanged from the pricing contract.
assert 'pub use veyndo_pricing::customer::Address;' in (ROOT/'components/shop/src/types/settings.rs').read_text()
assert 'struct Address' not in (ROOT/'components/shop/src/types/settings.rs').read_text()
assert 'struct Address' not in (ROOT/'src/commerce/types.rs').read_text()
address=(ROOT/'components/pricing/src/domain/customer/address.rs').read_text()
body=re.search(r'struct Address \{(.*?)\n}',address,re.S)[1]
expected={re.sub(r'_([a-z])',lambda m:m[1].upper(),f) for f in re.findall(r'pub (\w+):',body)}
assert set(order['properties']['billingAddress']['properties'])==expected
assert order['properties']['billingAddress']==order['properties']['shippingAddress']
print('PASS host contract fields agree with native order/line/amount/address DTOs and versioned job admission')
