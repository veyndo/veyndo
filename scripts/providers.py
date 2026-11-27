#!/usr/bin/env python3
"""Cloud wire-contract tests using local HTTP servers, NOT live cloud inference."""
from testing.app_approval import consent
from testing.database import psql
import json, copy, os, pathlib, subprocess, threading, time, urllib.request, urllib.error, uuid, concurrent.futures
from testing.native_tools import response as native_response
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

root = pathlib.Path(__file__).resolve().parents[1]
root.joinpath('.run').mkdir(exist_ok=True)
captured = []; checks = []; behavior = {'mode': 'normal'}; inference_started=threading.Event()
advisor_started=threading.Event();advisor_release=threading.Event()
proposal = {'summary':'HTTP provider contract test; not live inference.', 'changes':[{'product_id':'lamp','price':71.23}], 'operations':None, 'view':None, 'experience':None, 'expected_experience_revision':None}
def unsupported_unique(schema):
    if not isinstance(schema,dict): return False
    children=list(schema.get('properties',{}).values())
    if 'items' in schema: children.append(schema['items'])
    children+=schema.get('anyOf',[])+schema.get('oneOf',[])
    return 'uniqueItems' in schema or any(unsupported_unique(child) for child in children)
class Handler(BaseHTTPRequestHandler):
    def log_message(self,*args): pass
    def do_POST(self):
        body=json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        captured.append((self.path,{k.lower():v for k,v in self.headers.items()},body))
        if behavior['mode']=='slow':
            inference_started.set();time.sleep(1.5)
        if behavior['mode']=='reject':
            self.send_response(429); self.end_headers(); return
        result=proposal
        if behavior['mode']=='navigation-plan':result={**proposal,'changes':[],'navigation':behavior['navigation'],'view':behavior.get('navigationView')}
        if behavior['mode']=='view-plan':result={**proposal,'changes':[],'view':behavior['view']}
        if behavior['mode']=='work-plan':result={**proposal,'changes':[],'operations':behavior['operations']}
        if behavior['mode'] in ['extract','bad-extract']:
            quote='Ceramic capacity 500 ml.' if behavior['mode']=='extract' else 'Invented waterproof certification'
            result={'claims':[{'text':quote,'quote':quote,'nodeType':'property'}]}
        if behavior['mode'] in ['read-tools','write-tool','policy-read']:
            if behavior.setdefault('round',0)==0:
                result={**proposal,'changes':[],'tool_calls':[{'name':'merchant.commerce.read' if behavior['mode']=='policy-read' else 'merchant.orders' if behavior['mode']=='read-tools' else 'merchant.apply','arguments_json':'{"area":"aiPolicy"}' if behavior['mode']=='policy-read' else '{}'}]}
            behavior['round']+=1
        schema=(body.get('text',{}).get('format',{}).get('schema') or body.get('response_format',{}).get('json_schema',{}).get('schema') or body.get('output_config',{}).get('format',{}).get('schema') or body.get('format',{}))
        if self.path in ['/v1/responses','/v1/chat/completions'] and not body.get('tools') and unsupported_unique(schema):
            self.send_response(400);self.send_header('Content-Type','application/json');self.end_headers()
            self.wfile.write(json.dumps({'error':{'code':'invalid_json_schema','message':'uniqueItems is not permitted'}}).encode());return
        if body.get('tools'):
            result={'tool_calls':result.get('tool_calls',{}) if behavior['mode']=='invalid-read' else result.get('tool_calls',[])}
        elif behavior['mode']=='advisor':
            result={'explanation':'Synthetic consent-bound advice','recommended_ids':['lamp'],'layout':'discovery'}
            if behavior.get('advisor_block'):
                advisor_started.set()
                if not advisor_release.wait(10):
                    self.send_response(504);self.end_headers();return
        if self.path=='/v1/responses':
            answer={'status':'incomplete' if behavior['mode']=='incomplete' else 'completed','output':[{'type':'message','content':[{'type':'output_text','text':json.dumps(result)}]}], 'usage':{'input_tokens':10,'output_tokens':20}}
        elif self.path=='/v1/chat/completions':
            answer={'choices':[{'finish_reason':'stop','message':{'content':json.dumps(result)}}],'usage':{'prompt_tokens':10,'completion_tokens':20}}
        elif self.path=='/v1/messages':
            answer={'content':[{'type':'thinking','thinking':'contract fixture'},{'type':'text','text':json.dumps(result)}],'stop_reason':'end_turn','usage':{'input_tokens':10,'output_tokens':20}}
        else:
            self.send_response(404); self.end_headers(); return
        if body.get('tools'): answer=native_response(body,result,'anthropic' if self.path.endswith('/messages') else 'openai-chat' if self.path.endswith('/completions') else 'openai')
        self.send_response(200); self.send_header('Content-Type','application/json'); self.end_headers(); self.wfile.write(json.dumps(answer).encode())
server=ThreadingHTTPServer(('127.0.0.1',0),Handler)
threading.Thread(target=server.serve_forever,daemon=True).start()
port=server.server_address[1]
env={**os.environ,'OPENAI_API_KEY':'contract-openai','ANTHROPIC_API_KEY':'contract-claude','OPENAI_BASE_URL':f'http://127.0.0.1:{port}/v1','ANTHROPIC_BASE_URL':f'http://127.0.0.1:{port}/v1','FACT_SIGNING_SEED':'07'*32,'BIND_ADDR':'127.0.0.1:8789'}
log=open(root/'.run/provider-contract.log','w')
process=subprocess.Popen([str(root/'target/debug/veyndo')],cwd=root,env=env,stdout=log,stderr=log)
base='http://127.0.0.1:8789'; ah={'Authorization':'Bearer '+os.environ['MERCHANT_TOKEN'],'x-tenant':'workshop'}
def call(path,body=None,headers=None,expected=200,method=None):
    if path == '/api/apps' and isinstance(body,dict) and ('manifest' in body or 'builtIn' in body): body=consent(body)
    request=urllib.request.Request(base+path,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json',**(ah if headers is None else headers)},method=method)
    try:
        with urllib.request.urlopen(request,timeout=30) as r: status=r.status; value=json.load(r)
    except urllib.error.HTTPError as e: status=e.code; value=json.load(e)
    assert status==expected,(status,value)
    return value
def passed(name): checks.append(name); print('PASS',name)
try:
    for _ in range(60):
        if process.poll() is not None: raise RuntimeError('Contract app failed: inspect .run/provider-contract.log')
        try: call('/health'); break
        except OSError: time.sleep(.25)
    call('/api/agent/providers',headers={},expected=401); passed('Provider configuration requires merchant authentication')
    config=call('/api/agent/providers'); assert all(p['configured'] for p in config['providers']); assert 'contract-openai' not in json.dumps(config); passed('Credentials never returned to frontend')
    graph_app=json.loads((root/'extensions/apps/ontology-care/manifest.json').read_text());graph_app['id']='planner_ontology';graph_app['views']=[];graph_app['surfaces']=[];graph_app['apiRoutes']=[]
    graph_app['entities'][0]['fields']=[f for f in graph_app['entities'][0]['fields'] if f['name'] in ['title','product_id']]
    graph_app['intelligence']['ontology'][0]['fields']=['title','product_id'];graph_app['intelligence']['ontology'][0]['label']={'en':'Care'}
    graph_app['intelligence']['tools'].append('save_guides')
    call('/api/apps',{'manifest':graph_app})
    call('/api/apps/planner_ontology/entities/guides',{'id':'guide','fields':{'product_id':'mug','title':{'en':'Exact native care'}}})
    conversation=call('/api/agent/chat',{'message':'Contract test: change lamp price.','inference':{'provider':'openai','model':'contract-openai-model'}})
    cid=conversation['conversationId']; message=conversation['messages'][-1]; assert message['data'].get('preview',{}).get('inference')=='openai',conversation
    path,headers,body=next(c for c in reversed(captured) if c[0] != '/api/embed'); assert path=='/v1/responses' and headers['authorization']=='Bearer contract-openai' and body['store'] is False
    assert 'app.planner_ontology.care_advice' in body['input'] and 'app.planner_ontology.applies_to' in body['input'] and 'Exact native care' in body['input'], body['input']
    passed('Native app ontology fields and reference edges reach the actual merchant model request under the existing grants and bounded context')
    assert body['text']['format']['strict'] and body['model']=='contract-openai-model'; passed('OpenAI Responses adapter sends native strict schema and parses structured result')
    selection=body['text']['format']['schema']['properties']['translation_products']['anyOf'][0]
    assert 'uniqueItems' not in selection and selection['maxItems']==1000
    passed('Real planner final request accepts nullable product selection on a provider that rejects unsupported uniqueItems')
    task=message['data']['taskId']; result=call('/api/agent/tasks/'+task+'/apply',{'approve':True}); assert result['applied']
    restored=call('/api/agent/conversations/'+cid); assert restored['messages'][-1]['applied']; passed('Cloud-derived preview uses real approval transaction and persists applied state')
    conversation=call('/api/agent/chat',{'conversationId':cid,'message':'Tell me the previous task.','inference':{'provider':'anthropic','model':'contract-claude-model'}})
    path,headers,body=next(c for c in reversed(captured) if c[0] != '/api/embed'); assert path=='/v1/messages' and headers['x-api-key']=='contract-claude'
    assert body['output_config']['format']['type']=='json_schema' and 'Contract test' in body['messages'][0]['content']; passed('Claude Messages adapter preserves conversation context and ignores thinking blocks')
    call('/api/agent/conversations/'+cid,headers={**ah,'x-tenant':'atelier'},expected=404); passed('Conversation history cannot cross tenant boundary')
    behavior.update(mode='read-tools',round=0)
    planned=call('/api/agent/chat',{'message':'Read orders through the native tool first.','inference':{'provider':'openai'}})
    trace=planned['messages'][-1]['data']['preview']['toolTrace'];assert trace[0]['tool']=='merchant.orders' and behavior['round']==3
    assert 'gross EUR' not in next(c for c in reversed(captured) if c[0] != '/api/embed')[2]['instructions']
    assert captured[-2][2]['tools'] and 'text' not in captured[-2][2]
    assert 'Current native read transcript' in next(c for c in reversed(captured) if c[0] != '/api/embed')[2]['input']
    passed('Agent reads through the current capability dispatcher before a separate final proposal, with currency-neutral planning')
    behavior['mode']='invalid-read'
    malformed=call('/api/agent/chat',{'message':'Malformed read decision','inference':{'provider':'openai'}})
    assert malformed['messages'][-1]['data']['error'] and 'taskId' not in malformed['messages'][-1]['data']
    passed('Malformed tool decisions stop before reads or executable proposals')
    behavior.update(mode='write-tool',round=0)
    rejected=call('/api/agent/chat',{'message':'Unsupported direct write tool','inference':{'provider':'openai'}})
    assert rejected['messages'][-1]['data']['error'] and 'taskId' not in rejected['messages'][-1]['data']
    passed('Model-selected write tools cannot execute or create an executable task')
    from testing.app_discovery import verify_app_discovery
    verify_app_discovery(call, graph_app, ah, passed)
    from testing.agent_work import verify_agent_work
    verify_agent_work(call, behavior, ah, passed)
    from testing.merchant_guidance import verify_guidance
    verify_guidance(call, captured, behavior, passed)
    behavior['mode']='normal'
    from testing.extraction import verify_extraction
    from testing.provider_fleet import shared_provider
    with shared_provider(call, env['OPENAI_BASE_URL']):
        verify_extraction(call, captured, behavior, passed)
        from testing.advisor_privacy import verify_advisor_privacy
        verify_advisor_privacy(call, captured, behavior, advisor_started, advisor_release, passed)
        from testing.advisor_sources import verify_advisor_sources
        verify_advisor_sources(call, captured, behavior, advisor_started, advisor_release, passed)
    original=call('/api/merchant/commerce')
    def policy(value):
        snapshot=call('/api/merchant/commerce');data=copy.deepcopy(snapshot['data']);data['aiPolicy']=value
        return call('/api/merchant/commerce',{'revision':snapshot['revision'],'data':data},method='PUT')
    corridor={'productId':'lamp','currency':'EUR','minimumMinor':7000,'maximumMinor':8000,'minimumMarginBps':0}
    enabled={'corridors':[corridor], 'autonomy':{'enabled':True,'maxChangeBps':500,'productsPerDay':1}}
    policy(enabled)
    behavior.update(mode='policy-read',round=0)
    inspected=call('/api/agent/chat',{'message':'Read the saved AI price boundaries before reviewing lamp.','inference':{'provider':'openai'}})
    trace=inspected['messages'][-1]['data']['preview']['toolTrace']
    assert trace[0]['tool']=='merchant.commerce.read' and trace[0]['result']['data']['aiPolicy']==call('/api/merchant/commerce')['data']['aiPolicy'],trace
    final=next(c for c in reversed(captured) if c[0] != '/api/embed')[2]
    assert 'aiPolicy' in final['input'] and 'minimumMinor' in final['input'] and 'price-only autonomy' in final['instructions']
    behavior['mode']='normal'
    passed('Shared merchant agent reads current native AI boundaries through the authorized commerce capability before preparing a proposal')
    proposal['changes']=[{'product_id':'lamp','price':69}]
    rejected=call('/api/agent/chat',{'message':'Try outside price corridor','inference':{'provider':'openai'}})['messages'][-1]['data']
    simulation=rejected['preview']['simulation']
    assert simulation['blocking'] and any(r['rule']=='priceGuardrail' and 'corridorMinimum' in r['codes'] for r in simulation['rules']),simulation
    call('/api/agent/tasks/'+rejected['taskId']+'/apply',{'approve':True},expected=409)
    call('/api/intelligence/autonomy.apply',{'taskId':rejected['taskId']},expected=409)
    assert next(p for p in call('/api/search/product',{})['elements'] if p['id']=='lamp')['price']!=69
    passed('Merchant price corridors turn invalid previews into visible blocking findings; neither approval nor autonomy can execute them')
    proposal['changes']=[{'product_id':'lamp','price':72.5}]
    draft=call('/api/agent/chat',{'message':'Within explicit budget','inference':{'provider':'openai'}})['messages'][-1]['data']
    call('/api/intelligence/autonomy.apply',{'taskId':draft['taskId']},headers={**ah,'x-tenant':'atelier'},expected=404)
    assert call('/api/intelligence/autonomy.apply',{'taskId':draft['taskId']})['applied']
    assert call('/api/intelligence/autonomy.apply',{'taskId':draft['taskId']})['replayed']
    proposal['changes']=[{'product_id':'lamp','price':75}]
    draft=call('/api/agent/chat',{'message':'Compounding must fail','inference':{'provider':'openai'}})['messages'][-1]['data']
    call('/api/intelligence/autonomy.apply',{'taskId':draft['taskId']},expected=409)
    assert next(p for p in call('/api/search/product',{})['elements'] if p['id']=='lamp')['price']==72.5
    passed('Autonomy applies once within native settings and cannot compound beyond its original daily price baseline or affect foreign shops')
    # Daily exposure ceiling: reductions times current stock, summed over today's autonomous price rows.
    lamp=lambda: next(p for p in call('/api/search/product',{})['elements'] if p['id']=='lamp')
    def reduce(limit, expected=200):
        limited=copy.deepcopy(enabled);limited['autonomy']['maxDailyExposureMinor']=limit;policy(limited)
        proposal['changes']=[{'product_id':'lamp','price':70}]
        draft=call('/api/agent/chat',{'message':'Reduction against the exposure limit','inference':{'provider':'openai'}})['messages'][-1]['data']
        return call('/api/intelligence/autonomy.apply',{'taskId':draft['taskId']},expected=expected)
    reduce(1,409)
    status=call('/api/intelligence/autonomy.status',{})
    assert status['level']==1 and status['productsUsed']==1 and status['exposureMinor']==0 and status['maxDailyExposureMinor']==1,status
    baseline,stock=status['changes'][0]['baselineMinor'],lamp()['stock']
    need=(baseline-7000)*stock
    assert need>1 and status['changes'][0]['currentMinor']==7250 and lamp()['price']==72.5,(status,stock)
    reduce(need-1,409)
    assert lamp()['price']==72.5,'A refused autonomous reduction must leave the price unchanged'
    assert reduce(need)['applied'] and lamp()['price']==70
    status=call('/api/intelligence/autonomy.status',{})
    assert status['exposureMinor']==need and status['changes'][0]['exposureMinor']==need and status['changes'][0]['currentMinor']==7000,status
    tool=call('/mcp',{'jsonrpc':'2.0','id':7,'method':'tools/call','params':{'name':'knowledge.autonomy.status','arguments':{}}})
    assert tool['result']['structuredContent']['exposureMinor']==need,tool
    other=call('/api/intelligence/autonomy.status',{},headers={**ah,'x-tenant':'atelier'})
    assert other['changes']==[] and other['productsUsed']==0 and other['exposureMinor']==0,'Another shop must only see its own autopilot use'
    over=copy.deepcopy(enabled);over['autonomy']['maxDailyExposureMinor']=1_000_000_000_001
    snapshot=call('/api/merchant/commerce');data=copy.deepcopy(snapshot['data']);data['aiPolicy']=over
    call('/api/merchant/commerce',{'revision':snapshot['revision'],'data':data},method='PUT',expected=400)
    # Rollback: by request, and by the worker guard once today's exposure is above the limit.
    call('/api/intelligence/autonomy.revert',{},expected=400)
    back=call('/api/intelligence/autonomy.revert',{'approve':True})
    assert back['reverted']==['lamp'] and lamp()['price']==baseline/100,(back,lamp())
    assert call('/api/intelligence/autonomy.status',{})['exposureMinor']==0
    assert call('/api/intelligence/autonomy.revert',{'approve':True})['skipped'][0]['reason']=='at_baseline'
    assert reduce(need)['applied'] and lamp()['price']==70
    limited=copy.deepcopy(enabled);limited['autonomy']['maxDailyExposureMinor']=need-1;policy(limited)
    for _ in range(120):
        if lamp()['price']==baseline/100: break
        time.sleep(.5)
    assert lamp()['price']==baseline/100,'The worker guard must restore the day baseline once the limit is exceeded'
    # A price the merchant changed afterwards is never overwritten by a rollback.
    assert reduce(need)['applied']
    subprocess.check_output(psql(os.getenv('DB_CONTAINER','veyndo-postgres-1'),'commerce',os.getenv('TEST_DATABASE','commerce'),'-XqAt','-v','ON_ERROR_STOP=1'),input="BEGIN; SELECT set_config('rac.system','on',true); UPDATE products SET price=70.5,revision=revision+1 WHERE tenant='workshop' AND id='lamp'; COMMIT;",text=True)
    kept=call('/api/intelligence/autonomy.revert',{'approve':True,'productId':'lamp'})
    assert kept['reverted']==[] and kept['skipped'][0]['reason']=='changed_since' and lamp()['price']==70.5,kept
    policy(enabled)
    passed('Autonomy refuses price reductions beyond the daily exposure limit, applies at the exact limit, reports today\'s use over HTTP and MCP, restores the baseline by request and by the worker guard, and never overwrites a later manual price')
    proposal['changes']=[{'product_id':'lamp','price':73}]
    draft=call('/api/agent/chat',{'message':'Policy revision test','inference':{'provider':'openai'}})['messages'][-1]['data']
    locked=copy.deepcopy(enabled);locked['corridors'][0]['priceLocked']=True;policy(locked)
    call('/api/agent/tasks/'+draft['taskId']+'/apply',{'approve':True},expected=409)
    policy(original['data'].get('aiPolicy',{}))
    proposal['changes']=[{'product_id':'lamp','price':71.23}]
    draft=call('/api/agent/chat',{'message':'Restore synthetic fixture','inference':{'provider':'openai'}})['messages'][-1]['data']
    call('/api/agent/tasks/'+draft['taskId']+'/apply',{'approve':True})
    passed('Settings changes invalidate earlier proposals and applied state remains unchanged after denial')
    keys=call('/.well-known/commerce-facts.json',headers={})
    facts=call('/store-api/product/lamp/facts/signed',headers={'x-tenant':'workshop'})
    from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey
    import base64
    decode=lambda v:base64.urlsafe_b64decode(v+'='*((-len(v))%4))
    Ed25519PublicKey.from_public_bytes(decode(keys['keys'][0]['x'])).verify(decode(facts['signature']),decode(facts['payloadBase64']))
    assert json.loads(decode(facts['payloadBase64']))==facts['payload'] and facts['payload']['tenant']=='workshop'
    assert facts['payload']['expiresAt']-facts['payload']['issuedAt']==300
    assert call('/ucp/v1/products/lamp/facts',headers={'x-tenant':'workshop'})['payload']['tenant']=='workshop'
    passed('Real public and UCP-extension facts bind native product data and channel identity with independently verified Ed25519 signatures')
    # Signed cart offer: a buying agent can check the shop's quote offline and ask whether checkout would still confirm it.
    shop={'x-tenant':'workshop'}
    cart=call('/store-api/checkout/cart',{'session':'offer-'+uuid.uuid4().hex},shop);ch={**shop,'sw-context-token':cart['token']}
    call('/store-api/checkout/cart/offer',headers=ch,expected=409)
    call('/store-api/checkout/cart/offer',headers=shop,expected=401)
    quote=call('/store-api/checkout/cart/line-item',{'items':[{'referencedId':'lamp','quantity':2}]},ch)
    offer=call('/store-api/checkout/cart/offer',headers=ch)
    Ed25519PublicKey.from_public_bytes(decode(keys['keys'][0]['x'])).verify(decode(offer['signature']),decode(offer['payloadBase64']))
    signed=json.loads(decode(offer['payloadBase64']))
    assert signed==offer['payload'] and signed['kind']=='offer' and signed['tenant']=='workshop' and signed['currency']=='EUR' and signed['channel']=='default' and signed['totalMinor']==quote['price']['totalMinor'] and signed['cartRevision']==quote['revision'],signed
    assert signed['expiresAt']-signed['issuedAt']==600 and signed['lineItems']==[{'id':'lamp','label':quote['lineItems'][0]['label'],'quantity':2,'unitPrice':quote['lineItems'][0]['price']['unitPrice'],'totalPrice':quote['lineItems'][0]['price']['totalPrice']}],signed
    assert cart['token'] not in json.dumps(offer) and 'customerEmail' not in signed and 'stock' not in signed['lineItems'][0]
    proof={'payloadBase64':offer['payloadBase64'],'signature':offer['signature']}
    held=call('/store-api/checkout/offer/verify',proof,ch)
    assert held['valid'] and held['honoured'] and held['matchesCart'] is True and not held['expired'] and held['totalMinor']==signed['totalMinor'],held
    outside=call('/store-api/checkout/offer/verify',proof,shop)
    assert outside['valid'] and outside['matchesCart'] is None and not outside['honoured'],outside
    forged=base64.urlsafe_b64encode(json.dumps({**signed,'totalMinor':'1'}).encode()).decode().rstrip('=')
    assert call('/store-api/checkout/offer/verify',{**proof,'payloadBase64':forged},ch)=={'valid':False,'reason':'signature'}
    assert call('/store-api/checkout/offer/verify',proof,{'x-tenant':'atelier'})=={'valid':False,'reason':'other_shop'}
    assert call('/store-api/checkout/offer/verify',{'payloadBase64':'%%','signature':offer['signature']},ch)=={'valid':False,'reason':'malformed'}
    call('/store-api/checkout/offer/verify',{'signature':offer['signature']},ch,expected=400)
    tool=call('/mcp',{'jsonrpc':'2.0','id':11,'method':'tools/call','params':{'name':'offer.verify','arguments':proof}},ch)
    assert tool['result']['structuredContent']['honoured'] is True,tool
    tool=call('/mcp',{'jsonrpc':'2.0','id':12,'method':'tools/call','params':{'name':'cart.offer','arguments':{}}},ch)
    assert tool['result']['structuredContent']['payload']['totalMinor']==signed['totalMinor'],tool
    # Not a price lock: after the merchant changes the price the old offer is still authentic but no longer honoured.
    price=subprocess.check_output(psql(os.getenv('DB_CONTAINER','veyndo-postgres-1'),'commerce',os.getenv('TEST_DATABASE','commerce'),'-XqAt','-v','ON_ERROR_STOP=1'),input="BEGIN; SELECT set_config('rac.system','on',true); UPDATE products SET price=price+1,revision=revision+1 WHERE tenant='workshop' AND id='lamp'; COMMIT;",text=True)
    stale=call('/store-api/checkout/offer/verify',proof,ch)
    assert stale['valid'] and stale['matchesCart'] is False and not stale['honoured'],stale
    fresh=call('/store-api/checkout/cart/offer',headers=ch)['payload']
    assert int(fresh['totalMinor'])==int(signed['totalMinor'])+200 and fresh['cartRevision']==signed['cartRevision'],fresh
    subprocess.check_output(psql(os.getenv('DB_CONTAINER','veyndo-postgres-1'),'commerce',os.getenv('TEST_DATABASE','commerce'),'-XqAt','-v','ON_ERROR_STOP=1'),input="BEGIN; SELECT set_config('rac.system','on',true); UPDATE products SET price=price-1,revision=revision+1 WHERE tenant='workshop' AND id='lamp'; COMMIT;",text=True)
    call('/store-api/checkout/cart/line-item',{'items':[{'referencedId':'lamp','quantity':3}]},ch)
    changed=call('/store-api/checkout/offer/verify',proof,ch)
    assert changed['valid'] and changed['matchesCart'] is False and not changed['honoured'],changed
    passed('Signed cart offers verify independently, expose no cart capability or buyer data, and are honoured only for the same shop, cart revision and current total; forged, foreign and outdated offers are refused')
    # Poppy discovery: only the configured shop domain publishes the document, and the endpoint it names answers there.
    shop_host={'Host':'workshop.veyndo.ai'}
    poppy=call('/.well-known/poppy.json',headers=shop_host)
    assert set(poppy)=={'protocol_version','organization','apis'} and poppy['protocol_version']=='1.0',poppy
    assert poppy['organization']['domain']=='workshop.veyndo.ai' and poppy['organization']['name'].strip() and set(poppy['organization'])=={'name','domain'},poppy
    assert [(api['type'],api['url']) for api in poppy['apis']]==[('mcp','https://workshop.veyndo.ai/mcp')] and poppy['apis'][0]['description'].strip(),poppy
    for missing in [{}, {'x-tenant':'workshop'}, {'Host':'unknown-shop-for-poppy.veyndo.ai'}, {'Host':'workshop.previous.test'}]:
        call('/.well-known/poppy.json',headers=missing,expected=404)
    listed=call('/mcp',{'jsonrpc':'2.0','id':21,'method':'tools/list'},{**shop_host})
    assert {'catalog.search','cart.create','cart.offer','checkout.complete'}<={tool['name'] for tool in listed['result']['tools']},'The published MCP endpoint must offer guest shopping on the shop domain'
    assert call('/api/merchant/catalog/intake')['agentic']['poppy']=={'published':True,'url':'https://workshop.veyndo.ai/.well-known/poppy.json','version':'1.0','offers':['mcp']}
    passed('Poppy discovery is published only on the configured shop domain with matching domain and HTTPS MCP endpoint, and that endpoint lists guest shopping tools')
    # Trade network: a reorder proposal fetches a signed offer from another shop's public checkout; approval hands over, it never orders.
    supplier={'x-tenant':'atelier'}
    item=call('/store-api/product',{},supplier)['elements'][0]['id']
    orders_before=len(call('/api/merchant/orders',headers={**ah,'x-tenant':'atelier'}).get('elements'))
    def supply(value, expected=200):
        snapshot=call('/api/merchant/commerce');data=copy.deepcopy(snapshot['data']);data['supply']=value
        return call('/api/merchant/commerce',{'revision':snapshot['revision'],'data':data},method='PUT',expected=expected)
    link={'productId':'lamp','supplierShop':'atelier','supplierProductId':item,'reorderPoint':100000,'reorderQuantity':2}
    supply({'links':[link,link]},400)
    supply({'links':[{**link,'reorderQuantity':0}]},400)
    supply({'maxOrderMinor':1,'links':[link]})
    reorder=call('/api/merchant/supply/proposal',{})
    line=reorder['lines'][0];found=line['offer']
    Ed25519PublicKey.from_public_bytes(decode(keys['keys'][0]['x'])).verify(decode(found['signature']),decode(found['payloadBase64']))
    assert reorder['orders']=='none' and reorder['maxOrderMinor']==1 and line['productId']=='lamp' and line['quantity']==2 and line['withinCeiling'] is False,reorder
    assert found['payload']['tenant']=='atelier' and found['payload']['lineItems'][0]['id']==item and found['payload']['lineItems'][0]['quantity']==2,found
    ask={'productId':'lamp','payloadBase64':found['payloadBase64'],'signature':found['signature'],'cartToken':found['cartToken']}
    call('/api/merchant/supply/approve',ask,expected=400)
    call('/api/merchant/supply/approve',{**ask,'approve':True},expected=409)
    supply({'links':[link]})
    call('/api/merchant/supply/approve',{**ask,'approve':True},expected=409)
    supply({'maxOrderMinor':int(found['payload']['totalMinor']),'links':[link]})
    forged=base64.urlsafe_b64encode(json.dumps({**found['payload'],'totalMinor':'1'}).encode()).decode().rstrip('=')
    call('/api/merchant/supply/approve',{**ask,'payloadBase64':forged,'approve':True},expected=409)
    call('/api/merchant/supply/approve',{**ask,'productId':'mug','approve':True},expected=400)
    done=call('/api/merchant/supply/approve',{**ask,'approve':True})
    assert done['approved'] and done['ordered'] is False and done['checkoutPath'].startswith('/checkout?shop=atelier') and '#checkout/' in done['checkoutPath'] and done['expiresInSeconds']==600,done
    assert len(call('/api/merchant/orders',headers={**ah,'x-tenant':'atelier'}).get('elements'))==orders_before,'Approval must not place an order at the supplier'
    assert call('/api/merchant/supply/proposal',{})['lines'][0]['withinCeiling'] is True
    supply({'maxOrderMinor':100000,'links':[{**link,'supplierShop':'workshop'}]})
    own=call('/api/merchant/supply/proposal',{})['lines'][0]
    assert own['error']=='supplier_refused' and 'offer' not in own,own
    supply({'maxOrderMinor':100000,'links':[{**link,'reorderPoint':0}]})
    enough=call('/api/merchant/supply/proposal',{})
    assert enough['lines']==[] and enough['sufficient']==1,enough
    call('/api/merchant/supply/proposal',{},headers=supplier,expected=401)
    supply({'links':[]})
    passed('Reorder proposals carry an independently verifiable offer from another shop, respect the ceiling per reorder, refuse forged or unlinked requests, hand over to the supplier checkout without ordering and never treat a shop as its own supplier')
    request=urllib.request.Request(base+'/api/agent/chat/stream',data=json.dumps({'message':'Stream validated response','inference':{'provider':'openai'}}).encode(),headers={'Content-Type':'application/json',**ah})
    with urllib.request.urlopen(request,timeout=30) as response:
        frames=response.read().decode();assert response.headers['content-type'].startswith('text/event-stream') and 'event: accepted' in frames and 'event: complete' in frames
    passed('SSE returns acceptance and a native persisted chat completion through the same authorization owner')
    behavior['mode']='incomplete'
    v=call('/api/agent/chat',{'message':'incomplete','inference':{'provider':'openai'}}); assert v['messages'][-1]['data']['error'] and 'taskId' not in v['messages'][-1]['data']; passed('Incomplete OpenAI output creates explicit error without executable proposal')
    behavior['mode']='reject'
    v=call('/api/agent/chat',{'message':'rejected','inference':{'provider':'anthropic'}}); assert v['messages'][-1]['data']['error']; passed('Cloud HTTP rejection has no silent local-model fallback')
    if os.getenv('TEST_PERSONAL')=='1':
        fixture=json.loads((root/'.run/users-state.json').read_text()); owner=fixture['owner'];editor=fixture['editor'];other=fixture['other']
        def personal(s):return {'Authorization':'Bearer '+s['token'],'x-tenant':s['workspace']}
        invitation=call('/api/workspace/invitations',{'email':'plan-reader-'+uuid.uuid4().hex[:10]+'@example.test','role':'viewer'},personal(owner))
        reader=call('/api/auth/accept',{'invitationToken':invitation['token'],'name':'Plan Reader','password':'Synthetic-account-2026!'},headers={})
        before=next(p for p in call('/api/search/product',{},personal(other))['elements'] if p['id']=='lamp')['price']
        behavior['mode']='normal'
        planned=call('/api/agent/chat',{'message':'Create a price proposal; do not apply.','inference':{'provider':'openai','model':'contract-openai-model'}},personal(reader))
        task=planned['messages'][-1]['data']['taskId']
        call('/api/agent/tasks/'+task+'/apply',{'approve':True},personal(reader),403)
        call('/api/agent/tasks/'+task+'/apply',{'approve':True},personal(other),404)
        # A proposal is approved by the person who asked for it: an editor cannot execute a reader's plan, only their own.
        call('/api/agent/tasks/'+task+'/apply',{'approve':True},personal(editor),403)
        assert next(p for p in call('/api/search/product',{},personal(owner))['elements'] if p['id']=='lamp')['price']==before
        own=call('/api/agent/chat',{'message':'Create a price proposal; do not apply.','inference':{'provider':'openai','model':'contract-openai-model'}},personal(editor))['messages'][-1]['data']['taskId']
        assert call('/api/agent/tasks/'+own+'/apply',{'approve':True},personal(editor))['applied']
        assert next(p for p in call('/api/search/product',{},personal(owner))['elements'] if p['id']=='lamp')['price']==71.23
        assert next(p for p in call('/api/search/product',{},personal(other))['elements'] if p['id']=='lamp')['price']==before
        passed('Personal reader can plan but not execute; an editor executes only their own proposal in the owning shop')
        from testing.workspace_composition import verify_composition_access
        verify_composition_access(call, behavior, owner, personal, passed)
    from testing.copilot_navigation import verify_navigation
    verify_navigation(call, behavior, passed)
    behavior['mode']='normal'
    call('/api/apps',{'builtIn':'engraving'})
    before=call('/api/apps/engraving/entities/rules')['elements'][0]
    proposal['changes']=[];proposal['app_action']={'app':'engraving','action':'save_rules','arguments_json':json.dumps({'id':'default','fields':{'fee_minor':350}})}
    planned=call('/api/agent/chat',{'message':'Set engraving fee to 350 cents; preview only.','inference':{'provider':'openai'}})
    app_task=planned['messages'][-1]['data']['taskId']
    assert call('/api/apps/engraving/entities/rules')['elements'][0]['fee_minor']==before['fee_minor']
    assert call('/api/agent/tasks/'+app_task+'/apply',{'approve':True})['applied']
    after=call('/api/apps/engraving/entities/rules')['elements'][0];assert after['fee_minor']==350
    call('/api/agent/tasks/'+app_task+'/apply',{'approve':True});assert call('/api/apps/engraving/entities/rules')['elements'][0]['revision']==after['revision']
    passed('Registered app action uses grounded preview, explicit atomic approval and idempotent replay')
    proposal['app_action']['arguments_json']=json.dumps({'id':'default','fields':{'fee_minor':400}})
    planned=call('/api/agent/chat',{'message':'Preview another fee change.','inference':{'provider':'openai'}})
    app_task=planned['messages'][-1]['data']['taskId']
    call('/api/apps/engraving/entities/rules',{'id':'default','revision':after['revision'],'fields':{'fee_minor':450}})
    call('/api/agent/tasks/'+app_task+'/apply',{'approve':True},expected=409)
    assert call('/api/apps/engraving/entities/rules')['elements'][0]['fee_minor']==450
    passed('App record changed after preview rejects approval without partial writes')
    proposal.pop('app_action');behavior['mode']='slow';inference_started.clear()
    with concurrent.futures.ThreadPoolExecutor(max_workers=1) as executor:
        turn=executor.submit(call,'/api/agent/chat',{'conversationId':cid,'message':'Slow provider lease test','inference':{'provider':'openai'}})
        assert inference_started.wait(10)
        call('/api/agent/chat',{'conversationId':cid,'message':'Concurrent conflicting turn','inference':{'provider':'openai'}},expected=409)
        assert call('/health')['status']=='ok'
        idle=subprocess.check_output(psql(os.getenv('DB_CONTAINER','veyndo-postgres-1'),'commerce',os.getenv('TEST_DATABASE','commerce'),'-Atc',"SELECT count(*) FROM pg_stat_activity WHERE state='idle in transaction' AND query LIKE '%pg_try_advisory_xact_lock%'"),text=True).strip()
        assert idle=='0';assert turn.result()['messages'][-1]['data']['taskId']
    behavior['mode']='normal'
    passed('Conversation lease excludes concurrent turns while inference holds no advisory transaction or DB connection')
    process.terminate();process.wait(timeout=15)
    env.update(OPENAI_PROTOCOL='chat',INFERENCE_MODEL_PLANNER_OPENAI='self-hosted-planner',INFERENCE_PROMPT_CACHE='true')
    process=subprocess.Popen([str(root/'target/debug/veyndo')],cwd=root,env=env,stdout=log,stderr=log)
    for _ in range(60):
        try:call('/health');break
        except OSError:time.sleep(.1)
    streamed=call('/api/agent/chat',{'message':'Use routed self-hosted planner','inference':{'provider':'openai'}})
    path,_,body=next(c for c in reversed(captured) if c[0] != '/api/embed');assert path=='/v1/chat/completions' and body['model']=='self-hosted-planner' and body['response_format']['json_schema']['strict']
    assert not streamed['messages'][-1]['data'].get('error')
    call('/api/agent/chat',{'message':'Explicit model still wins','inference':{'provider':'openai','model':'chosen-model'}})
    assert next(c for c in reversed(captured) if c[0] != '/api/embed')[2]['model']=='chosen-model'
    call('/api/agent/chat',{'message':'Stable Claude system cache','inference':{'provider':'anthropic'}})
    assert next(c for c in reversed(captured) if c[0] != '/api/embed')[2]['system'][0]['cache_control']=={'type':'ephemeral'}
    passed('Self-hosted chat-completions schema, task model routing, explicit-model precedence and Claude prompt-cache contract work through real HTTP')
    report={'passed':len(checks),'liveCloudInference':False,'checks':checks}
    print(json.dumps(report,indent=2))
    if os.environ.get('REPORT_PATH'): pathlib.Path(os.environ['REPORT_PATH']).write_text(json.dumps(report,indent=2)+'\n')
finally:
    process.terminate(); process.wait(timeout=15); log.close(); server.shutdown()
