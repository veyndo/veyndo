#!/usr/bin/env python3
"""Signed app packages as agent tools: effects on MCP tools, refused unauthorized calls, typed port wiring and version refusal, contract tamper refusal, upgrade/rollback with a data migration and Wasm hot swap without restart."""
import base64,copy,json,os,socket,subprocess,tempfile,urllib.request,urllib.error,uuid
from testing.runtime import ROOT,serve,stop
from testing.app_approval import consent
seed=base64.b64encode(bytes([11])*32).decode()
def signed(manifest,contract=None):
    """Signs with the real offline CLI; the seed only ever travels on stdin."""
    with tempfile.NamedTemporaryFile(mode='w',suffix='.json') as m,tempfile.NamedTemporaryFile(mode='w',suffix='.json') as c:
        json.dump(manifest,m);m.flush();args=[str(ROOT/'target/debug/veyndo'),'--sign-app',m.name]
        if contract is not None:json.dump(contract,c);c.flush();args.append(c.name)
        return json.loads(subprocess.check_output(args,input=seed,text=True))
def model(app,version,fields,actions,migrations=None,signed_by=True):
    m={'id':app,'version':version,'coreApi':'1','runtime':'declarative','name':{'en':app,'de':app,'fr':app,'es':app},
       'permissions':['data.read','data.write'],'entities':[{'name':'ledger','fields':fields,'publicRead':False}],'slots':[],'actions':actions,'events':[]}
    if migrations:m['schemaMigrations']=migrations
    if signed_by:m['distribution']={'publisher':'fixture','keyId':'first','signature':'','channel':'stable','dependencies':[]}
    return m
LIST={'name':'entries','description':'Read ledger entries','handler':'list','entity':'ledger','inputSchema':{'type':'object','properties':{},'additionalProperties':False},'outputSchema':{'type':'object'},'public':False}
SAVE={'name':'save_entry','description':'Write a ledger entry','handler':'save','entity':'ledger','inputSchema':{'type':'object','properties':{'id':{'type':'string'},'revision':{'type':'integer'},'fields':{'type':'object'}},'required':['id','fields'],'additionalProperties':False},'public':False}
def port(version):return {'name':'fixture.ledger','version':version,'action':'entries','inputSchema':LIST['inputSchema'],'outputSchema':LIST['outputSchema']}
def contract(app,version,**extra):return {'app':app,'version':version,**extra}
keys=signed(model('fixture_probe','1.0.0',[{'name':'label','kind':'string','required':False,'indexed':False}],[LIST]))['publicKey']
with socket.socket() as probe:probe.bind(('127.0.0.1',0));port_number=probe.getsockname()[1]
base=f'http://127.0.0.1:{port_number}';env={**os.environ,'BIND_ADDR':f'127.0.0.1:{port_number}','APP_PUBLISHERS':json.dumps({'fixture':{'first':keys}})}
def call(path,body=None,h=None,status=200,method=None):
    if path=='/api/apps' and body is not None:body=consent(body)
    req=urllib.request.Request(base+path,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json',**(h or {})},method=method)
    try:
        with urllib.request.urlopen(req,timeout=30) as r:code=r.status;value=json.load(r)
    except urllib.error.HTTPError as e:code=e.code;value=json.load(e)
    assert code==status,(path,code,status,value);return value
def owner():
    t='agentpkg-'+uuid.uuid4().hex[:10];u=call('/api/auth/register',{'workspaceId':t,'name':'Package fixture','email':t+'@example.test','password':'Synthetic-package-password!'})
    return {'x-tenant':t,'Authorization':'Bearer '+u['token']}
def tools(h):return {t['name']:t for t in call('/mcp',{'jsonrpc':'2.0','id':1,'method':'tools/list'},h)['result']['tools']}
def install(m,c,h,status=200):
    body={'manifest':m} if c is None else {'manifest':m,'contract':c}
    return call('/api/apps',body,h,status)
process=None
try:
    with open(ROOT/'.run/app-agent-packages.log','wb') as log:process=serve(env,base,log)
    h=owner();foreign=owner()

    # 1. Installed app operations are MCP tools with planning effects; undescribed writes are conservative.
    call('/api/apps',{'builtIn':'engraving'},h)
    listed=tools(h);rules=listed['app.engraving.rules'];save=listed['app.engraving.save_rules']
    assert rules['facts']=={'audience':'merchant','permissions':[],'writes':False,'reversibility':{'kind':'reversible'},'risk':'low','data':{'entities':['app'],'pii':False,'money':True,'stock':False},'effects':[],'requires':[],'conditions':[]},rules
    assert rules['cost']=='free' and rules['requiresApproval'] is False and rules['declared'] is True
    assert save['facts']['writes'] and save['facts']['risk']=='medium' and save['facts']['requires']==['app.engraving.rules'],save
    assert save['facts']['permissions']==['catalog'] and 'cart' in save['facts']['data']['entities'] and save['cost']=='standard'
    assert save['requiresApproval'] is False and save['annotations']['readOnlyHint'] is False
    local=model('notes_'+uuid.uuid4().hex[:6],'1.0.0',[{'name':'label','kind':'string','required':False,'indexed':False}],[LIST,SAVE],signed_by=False)
    install(local,None,h)
    plain=tools(h)[f"app.{local['id']}.save_entry"]
    assert plain['facts']['risk']=='high' and plain['facts']['reversibility']=={'kind':'irreversible'} and plain['requiresApproval'] and not plain['declared'],plain
    print('PASS installed app operations are MCP tools with native-shaped planner facts; an undescribed write is high risk and needs approval')

    # 2. A call without the actor's rights is refused by the gateway, never routed around it.
    anonymous={'x-tenant':h['x-tenant']}
    assert not any(n.startswith('app.') for n in tools(anonymous))
    refused=call('/mcp',{'jsonrpc':'2.0','id':2,'method':'tools/call','params':{'name':'app.engraving.save_rules','arguments':{'id':'default','fields':{'fee_minor':1}}}},anonymous)
    assert 'error' in refused or refused['result'].get('isError'),refused
    call('/api/apps/engraving/actions/save_rules',{'id':'default','revision':1,'fields':{'fee_minor':1}},{**foreign,'x-tenant':h['x-tenant']},status=403)
    assert call('/api/apps/engraving/entities/rules',h=h)['elements'][0]['fee_minor']==300
    print('PASS an unauthorized MCP or HTTP call of an app tool is refused and changes nothing')

    # 3. Typed ports: a consumer needs a compatible provider; incompatible versions are refused at install.
    fields=[{'name':'label','kind':'string','required':False,'indexed':False},{'name':'amount','kind':'integer','required':False,'indexed':False}]
    shop=model('fixture_shop','1.0.0',fields,[LIST]);shop_c=contract('fixture_shop','1.0.0',consumes=[{'name':'fixture.ledger','version':'^1.0'}])
    s=signed(shop,shop_c);install(s['manifest'],s['contract'],h,409)
    ledger=model('fixture_ledger','1.0.0',fields,[LIST,SAVE]);ledger_c=contract('fixture_ledger','1.0.0',provides=[port('1.0.0')])
    v1=signed(ledger,ledger_c);install(v1['manifest'],v1['contract'],h)
    install(s['manifest'],s['contract'],h)
    call('/api/apps/fixture_ledger/entities/ledger',{'id':'first','revision':0,'fields':{'label':'Opening','amount':1200}},h)
    wired=call('/api/apps/fixture_shop/ports/fixture.ledger',{},h)
    assert wired['provider']=='fixture_ledger' and wired['version']=='1.0.0' and wired['result']['elements'][0]['amount']==1200,wired
    call('/api/apps/fixture_shop/ports/fixture.other',{},h,403)
    call('/api/apps/fixture_shop/ports/fixture.ledger',{},{**foreign,'x-tenant':h['x-tenant']},403)
    too_new=signed(model('fixture_shop','1.1.0',fields,[LIST]),contract('fixture_shop','1.1.0',consumes=[{'name':'fixture.ledger','version':'^2'}]))
    install(too_new['manifest'],too_new['contract'],h,409)
    major=signed(model('fixture_ledger','2.0.0',fields,[LIST,SAVE]),contract('fixture_ledger','2.0.0',provides=[port('2.0.0')]))
    install(major['manifest'],major['contract'],h,409)
    call('/api/apps/fixture_ledger',{'active':False,'revision':1},h,409,'PUT')
    sandbox=call('/api/apps/fixture_shop/sandbox',h=h)
    assert sandbox['declared']['ports']['consumes']==[{'name':'fixture.ledger','version':'^1.0','provider':{'app':'fixture_ledger','version':'1.0.0'}}],sandbox
    print('PASS ports wire a consumer to the installed provider; missing, too-new and stranding versions are refused at install and deactivation')

    # 4. Signatures: the contract is bound to the signed manifest; tampering with either is refused.
    tampered=copy.deepcopy(v1['contract']);tampered['provides'][0]['version']='1.0.1'
    install(v1['manifest'],tampered,h,403)
    install(v1['manifest'],{**v1['contract'],'signature':None},h,403)
    other=copy.deepcopy(v1['manifest']);other['permissions'].append('customers.pii')
    install(other,v1['contract'],h,403)
    review=call('/api/apps/review',{'manifest':v1['manifest'],'contract':v1['contract']},h)
    install(v1['manifest'],v1['contract'],{**h},200)
    assert review['contractDigest'] and review['contract']['provides'][0]['name']=='fixture.ledger'
    stale=consent({'manifest':v1['manifest'],'contract':v1['contract']});stale['contractDigest']='0'*64
    call('/api/apps',stale,h,409)
    print('PASS a tampered contract, an unsigned contract of a signed package and a changed manifest are refused; review binds the contract digest')

    # 5. Upgrade and explicit rollback with a per-version data migration, no restart.
    renamed=[fields[0],{'name':'amount_minor','kind':'integer','required':False,'indexed':False}]
    up=model('fixture_ledger','1.1.0',renamed,[LIST,SAVE],migrations=[{'fromVersion':'1.0.0','steps':[{'op':'rename','entity':'ledger','field':'amount','to':'amount_minor'}]}])
    up_c=contract('fixture_ledger','1.1.0',provides=[port('1.1.0')],rollbacks=[{'toVersion':'1.0.0','steps':[{'op':'rename','entity':'ledger','field':'amount_minor','to':'amount'}]}])
    v2=signed(up,up_c);install(v2['manifest'],v2['contract'],h)
    row=call('/api/apps/fixture_ledger/entities/ledger',h=h)['elements'][0];assert row['amount_minor']==1200 and 'amount' not in row,row
    assert call('/api/apps/fixture_shop/ports/fixture.ledger',{},h)['version']=='1.1.0'
    install(v1['manifest'],v1['contract'],h,409)
    call('/api/apps/fixture_ledger/rollback',{'version':'1.0.0'},h,400)
    call('/api/apps/fixture_ledger/rollback',{'version':'0.9.0','approve':True},h,400)
    sandbox=call('/api/apps/fixture_ledger/sandbox',h=h)
    assert [v['version'] for v in sandbox['versions'] if v['rollback']]==['1.0.0'],sandbox['versions']
    assert any(w['entity']=='ledger' and w['record']=='first' for w in sandbox['observed']['writes']),sandbox['observed']
    back=call('/api/apps/fixture_ledger/rollback',{'version':'1.0.0','approve':True},h)
    assert back['version']=='1.0.0',back
    row=call('/api/apps/fixture_ledger/entities/ledger',h=h)['elements'][0];assert row['amount']==1200 and 'amount_minor' not in row,row
    call('/api/apps/fixture_ledger/rollback',{'version':'1.1.0','approve':True},h,409)
    install(v2['manifest'],v2['contract'],h)
    assert call('/api/apps/fixture_ledger/entities/ledger',h=h)['elements'][0]['amount_minor']==1200
    print('PASS upgrade migrates app data, a plain downgrade is refused, the explicit rollback converts data back and the same version upgrades again')

    # 6. Hot swap: a new Wasm module serves the next call without a restart.
    engraving=json.loads((ROOT/'extensions/apps/engraving/manifest.json').read_text())
    hot=copy.deepcopy(engraving);hot['id']='hotswap_'+uuid.uuid4().hex[:6];hot['version']='1.0.0'
    install(hot,None,foreign)
    def fee():
        ch={'x-tenant':foreign['x-tenant']};c=call('/store-api/checkout/cart',{'session':uuid.uuid4().hex},ch);ch['sw-context-token']=c['token']
        c=call(f"/store-api/apps/{hot['id']}/configure",{'productId':'mug','fields':{'text':'Ada'},'revision':c['revision']},ch)
        c=call('/store-api/checkout/cart/line-item',{'items':[{'referencedId':'mug','quantity':1}]},ch)
        return next(i for i in c['lineItems'] if i['id']=='mug')['configuration']['feeMinor']
    assert fee()==300
    swapped=copy.deepcopy(hot);swapped['version']='1.1.0'
    swapped['configuration']['wasmSource']=hot['configuration']['wasmSource'].replace('if (result i64) local.get $fee else','if (result i64) local.get $fee i64.const 2 i64.mul else')
    install(swapped,None,foreign)
    assert fee()==600
    assert process.poll() is None
    print('PASS an upgrade swaps the Wasm module for the next call on the running server')
finally:
    if process is not None:stop(process)
