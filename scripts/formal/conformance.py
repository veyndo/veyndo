"""Compare compiled Rust production policies with Lean's extracted executable,
including every Boolean input and finite numeric edge/random cases. Tests are
bridge evidence; universal claims come from the kernel-checked theorems.
"""
import itertools,json,random,subprocess,tempfile
from extract import MAX_U64

SEED=20261002
EDGES=[0,1,2,16,17,99,100,9999,10000,10001,2**31-1,2**31,2**63-1,2**63,MAX_U64-1,MAX_U64]

def cases(functions):
    """The generated input population: every Boolean assignment crossed with numeric edges, plus seeded random cases."""
    result=[];rng=random.Random(SEED)
    for f in functions:
        options=[[False,True] if t=='bool' else EDGES for _,t in f.args]
        for values in itertools.product(*options):
            result.append({'function':f.name,'args':dict(zip((n for n,_ in f.args),values))})
        if f.name=='external_amount_admissible':
            # Exact application caps and neighbours, including the zero-rate tax condition.
            for gross,tax,rate in [(10**12-1,0,99999),(10**12,10**12,100000),
                (10**12+1,0,1),(1,0,100001),(1,1,0),(1,2,1),(0,0,0)]:
                result.append({'function':f.name,'args':{'gross':gross,'tax':tax,'rate':rate}})
        if any(t=='u64' for _,t in f.args):
            for _ in range(250):
                result.append({'function':f.name,'args':{n:rng.getrandbits(64) if t=='u64' else bool(rng.getrandbits(1)) for n,t in f.args}})
    return result

def evaluate(command,population,cwd=None,env=None,timeout=120):
    """Feed one JSON request per line and return the decoded answers in order."""
    source=''.join(json.dumps(c,separators=(',',':'))+'\n' for c in population)
    # Large bidirectional pipes can stall the native Lean reader on macOS.
    # A seekable input keeps the exact case population and output comparison.
    with tempfile.TemporaryFile(mode='w+t') as requests:
        requests.write(source);requests.seek(0)
        run=subprocess.run(command,stdin=requests,capture_output=True,text=True,check=True,timeout=timeout,cwd=cwd,env=env)
    answers=[json.loads(line)for line in run.stdout.splitlines()]
    if len(answers)!=len(population):raise ValueError('Conformance driver dropped cases')
    return answers

def mismatches(population,rust,lean):
    return [case for case,r,l in zip(population,rust,lean) if type(r)!=type(l) or r!=l]

def compare(root, functions):
    population=cases(functions)
    rust=evaluate([str(root/'target/debug/verified_kernel')],population)
    lean=evaluate([str(root/'proof/.lake/build/bin/conformance')],population)
    wrong=mismatches(population,rust,lean)
    if wrong:raise ValueError(f'Rust/Lean mismatch: {wrong[0]}')
    return {'cases':len(population),'mismatches':0,'allBooleanAssignments':True,'numericBoundaryIncludesU64Max':True,'seed':SEED}
