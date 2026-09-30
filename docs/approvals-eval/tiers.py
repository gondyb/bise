import json, os, re, sys, collections
HOME=os.path.expanduser('~')
# each hub's workspace (edit for your machine)
ROOTS={'harness-3abb2bd8':HOME+'/lab/bend-lab/harness','dashboard-e95755e1':HOME+'/mistral/dashboard'}
TMP_IS_ROOT = os.environ.get('TMP_ROOT','1')=='1'   # after the TMPDIR steering, /tmp writes go to ~/.bise/tmp/<id>
READ_ANYWHERE = os.environ.get('READ_ANY','1')=='1' # auto: a read outside the roots is allowed unless it hits a secret path
PROTECTED=[HOME+'/.bise/hubs', HOME+'/.bise/approvals.toml', HOME+'/.bise/auth.json']
SECRET=re.compile(r'(^|/)(\.ssh|\.aws|\.gnupg|\.netrc|\.npmrc|\.env(\.[\w.]+)?|auth\.json|credentials|id_rsa|id_ed25519)(/|$)')
READ={'cat','head','tail','ls','wc','grep','rg','find','stat','file','diff','sort','uniq','cut','tr','jq','pwd','which','date','basename','dirname','readlink','realpath','du','df','shasum','sha256sum','md5','tree','echo','printf','nl','column','xxd','od','hexdump','strings','fold','comm','paste','expand','tac','rev','true','false','test','[','sleep','type','seq','printenv','whoami','uname','hostname','id','ps','pgrep','lsof','cmp','fmt','bat','fd','ag','less','more','awk','sed','yes','wait','command','hash','ulimit','tput','stty','locale','sw_vers','sysctl','cal','bc','expr','git-blame','file'}
BUILTIN={'break','continue','wait','mktemp','cd','pushd','popd','export','unset','set','shift','local','read','exit','return',':','trap','declare','typeset','alias','shopt','source','.'}
GIT_READ={'status','log','diff','show','branch','rev-parse','ls-files','blame','grep','remote','cat-file','merge-base','describe','shortlog','reflog','ls-tree','for-each-ref','rev-list','name-rev','config','worktree','stash','tag','fetch','check-ignore','var','help','version','--version','count-objects','fsck','show-ref','symbolic-ref','whatchanged','range-diff','cherry','notes','diff-tree','diff-index','diff-files'}
GUARD={'find':{'-exec','-execdir','-delete','-ok','-okdir','-fprint','-fprintf','-fls'},'sort':{'-o','--output'},'rg':{'--pre'},'git':{'--output','--ext-diff','--textconv','-c','--exec'}}
GIT_LOCAL={'add','commit','write-tree','read-tree','hash-object','update-index','commit-tree','mktree','apply','diff-index','check-attr','ls-remote','am','format-patch'}
WRITERS={'mkdir','touch','cp','mv','rm','ln','truncate','tee','chmod','rmdir','install','rsync'}
WRAP={'env','time','nohup','nice','timeout','command','caffeinate','stdbuf','xargs','exec'}
INTERP={'python','python3','node','bun','deno','perl','ruby','php','osascript','bash','sh','zsh','fish','uv','npx','bunx','tsx','ts-node'}
ARITY={'cargo':2,'git':2,'npm':2,'pnpm':2,'yarn':2,'bun':2,'make':2,'gh':3,'docker':2,'uv':2,'go':2,'bend':2,'sb':2,'bise':2,'brew':2,'kubectl':2,'pip':2,'pytest':1,'node':2,'python3':2,'python':2,'rustup':2,'tmux':2,'curl':1,'open':1}
ARITY3={('npm','run'),('pnpm','run'),('yarn','run'),('bun','run'),('cargo','run'),('uv','run'),('docker','compose'),('git','stash'),('gh','pr'),('gh','issue'),('gh','api'),('gh','run')}

def norm(path, base):
    p=path
    if p.startswith('~'): p=HOME+p[1:]
    if not p.startswith('/'): p=os.path.join(base,p)
    return os.path.normpath(p)
def in_roots(p, root):
    if p=='/dev/null' or p.startswith('/dev/fd') or p in ('/dev/stdout','/dev/stderr'): return True
    if any(p==x or p.startswith(x+'/') for x in PROTECTED): return False
    for r in (root, HOME+'/.bise'):
        if p==r or p.startswith(r+'/'): return True
    if TMP_IS_ROOT and (p=='/tmp' or p.startswith('/tmp/') or p.startswith('/private/tmp/') or p.startswith('/var/folders/')): return True
    return False
def protected(p): return any(p==x or p.startswith(x+'/') for x in PROTECTED) or '/.git/' in p+'/' and not p.endswith('/.git')

def strip_wrappers(w):
    while w:
        h=os.path.basename(w[0])
        if h in ('env',): 
            w=w[1:]
            while w and (('=' in w[0] and not w[0].startswith('-')) or w[0] in ('-i','-u') or (len(w)>0 and w[0].startswith('-'))):
                w = w[2:] if w[0]=='-u' else w[1:]
            continue
        if h in ('time','nohup','caffeinate','command','exec','stdbuf'): w=[x for x in w[1:]] ; 
        elif h=='timeout': w=w[1:]; w=[x for i,x in enumerate(w) if not (i==0 and (x[0].isdigit() or x.startswith('-')))]
        elif h=='nice': w=w[1:]; w = w[2:] if w and w[0]=='-n' else w
        elif h=='xargs':
            w=w[1:]
            while w and w[0].startswith('-'):
                w = w[2:] if w[0] in ('-I','-n','-P','-L','-d','-s','-E') else w[1:]
            if not w: return ['echo']
        else: break
    return w

def pattern(w):
    prog=os.path.basename(w[0]); a=ARITY.get(prog,1)
    if len(w)>1 and (prog,w[1]) in ARITY3: a=3
    return ' '.join([prog]+w[1:a])+' *'

def judge_part(p, base, root):
    """-> (verdict, why, key, base) verdict in allow|classify|card"""
    w=p['words']; dyn=p.get('dyn',[False]*len(w))
    if w and w[0]=='<unparsed>': return 'classify','unparsed',None,base
    # redirections
    for r in p.get('redirs',[]):
        if r['kind']=='file' and r['op'] in ('>','>>','>|','&>','&>>','<>'):
            if r.get('target','').find('$')>=0 or p.get('rdyn'): return 'classify','dynamic redirect',None,base
            t=norm(r['target'],base)
            if protected(t): return 'card','protected write '+t,None,base
            if not in_roots(t,root): return 'classify','write outside roots '+t,None,base
    if w and w[0] in ('<compound>','<function>','[[','(('): return 'allow','compound',None,base
    ww=[x for x in w if x!='']; 
    if not ww: return 'allow','assign only',None,base
    ww=strip_wrappers(ww); 
    if not ww: return 'allow','wrapper',None,base
    prog=os.path.basename(ww[0]); args=ww[1:]
    if prog=='git':
        while args and args[0] in ('-C','--git-dir','--work-tree','--no-pager','-P','--no-optional-locks'):
            args = args[2:] if args[0] in ('-C','--git-dir','--work-tree') else args[1:]
        ww=['git']+args
        if args and args[0] in GIT_LOCAL and not any(a in ('--force','-f','--hard') for a in args): return 'allow','git local',None,base
    if any(dyn[:1]) or '$' in ww[0]: return 'classify','dynamic program',None,base
    text=' '.join(ww)
    # hard rules
    if prog in ('sudo','doas','su'): return 'card','sudo',None,base
    if prog=='git' and args[:1]==['push'] and (any(a in ('--force','-f','--force-with-lease') or a.startswith('+') for a in args) or any(re.search(r'(^|:)(main|master)$',a) for a in args[1:])): return 'card','push to main / force',None,base
    if prog=='rm' and any(a.startswith('-') and 'r' in a.lower() for a in args):
        for a in args:
            if not a.startswith('-'):
                t=norm(a,base)
                if t in (root, HOME, HOME+'/.bise', '/') : return 'card','rm -rf a root',None,base
    if prog=='sb': return 'allow','sb',None,base
    if prog in ('cd','pushd'):
        if args and not any(dyn[1:2]): base=norm(args[0],base)
        return 'allow','cd',None,base
    if prog in BUILTIN and prog not in ('source','.'): return 'allow','builtin',None,base
    anydyn=any(dyn[1:])
    # reads
    if prog in READ or (prog=='git' and args and args[0] in GIT_READ):
        g=GUARD.get(prog,set())
        if any(a in g or a.split('=')[0] in g for a in args): return 'classify','read with a guarded option',text,base
        if prog=='sed' and any(a=='-i' or a.startswith('-i') or a.startswith('--in-place') for a in args):
            pass  # a write, below
        elif prog=='awk' and any(re.search(r'system\(|\|\s*"|print[^;]*>\s*"?[\w/]',a) for a in args): return 'classify','awk writes/execs',text,base
        elif prog=='git' and args[0] in ('branch','tag','stash','worktree','config','remote','notes','fetch') and not (len(args)==1 or args[1] in ('-a','-l','--list','-v','-vv','--show-current','list','show','--get','--get-regexp','--contains','-r')):
            return 'classify','git ref change', pattern(ww), base
        else:
            paths=[a for a in args if not a.startswith('-') and ('/' in a or a.startswith('~') or a.startswith('.'))]
            for a in paths:
                t=norm(a,base)
                if SECRET.search(t): return 'classify','secret read '+t,text,base
                if not READ_ANYWHERE and not in_roots(t,root): return 'classify','read outside roots',text,base
            return 'allow','read',None,base
    # plain writes
    if prog in WRITERS or prog=='sed':
        if anydyn: return 'classify','dynamic write target',text,base
        targets=[a for a in args if not a.startswith('-')]
        if prog=='sed': targets=targets[1:]
        if prog=='chmod': targets=targets[1:]
        for a in targets:
            t=norm(a,base)
            if protected(t): return 'card','protected write',None,base
            if not in_roots(t,root): return 'classify','write outside roots '+t,text,base
        return 'allow','plain write',None,base
    # interpreters fed inline code: exact text, never cached by pattern
    if prog in INTERP:
        if any(a in ('-','-c','-e','-p','--eval') for a in args[:3]) or not args or any(r['kind']=='heredoc' for r in p.get('redirs',[])):
            return 'classify','inline code',text+'|'+str(len(text)),base
    if anydyn: return 'classify','dynamic args',text,base
    return 'classify','opaque',pattern(ww),base

def judge(parsed, root):
    base=root; outs=[]
    for p in parsed['parts']:
        v,why,key,base=judge_part(p,base,root)
        outs.append((v,why,key,' '.join(p['words'][:4])))
    if any(o[0]=='card' for o in outs): return 'card',outs
    if any(o[0]=='classify' for o in outs): return 'classify',outs
    return 'allow',outs

meta={}
for l in open('bash_meta.jsonl'):
    d=json.loads(l); meta[d['i']]=d
res=[]
for l in open('bash_parsed.jsonl'):
    d=json.loads(l); m=meta[d['i']]
    v,outs=judge(d, ROOTS[m['hub']])
    res.append((m,v,outs))
N=len(res); c=collections.Counter(v for _,v,_ in res)
print('commands',N, dict(c), {k:round(100*x/N,1) for k,x in c.items()})
why=collections.Counter(o[1].split(' /')[0].split(' ~')[0] if not o[1].startswith('write outside') else 'write outside roots' for _,v,outs in res if v=='classify' for o in outs if o[0]=='classify')
print('classify reasons (parts):', why.most_common(12))
# cache simulation
def keys(outs, mode):
    ks=[]
    for v,why,key,_ in outs:
        if v!='classify': continue
        if key is None: ks.append(None)   # never cacheable
        elif mode=='exact': ks.append(key if why!='opaque' else key)  # key already pattern for opaque
        else: ks.append(key)
    return ks
for scope in ('agent','repo'):
  for mode in ('none','exact','pattern'):
    seen=collections.defaultdict(set); calls=0
    for m,v,outs in res:
        if v!='classify': continue
        sc=m['agent'] if scope=='agent' else m['hub']
        ks=[]
        for vv,why,key,t in outs:
            if vv!='classify': continue
            if mode=='none' or key is None: ks.append(None)
            elif mode=='exact': ks.append(t if why=='opaque' else key)  # exact text of the part
            else: ks.append(key)
        if any(k is None or k not in seen[sc] for k in ks):
            calls+=1; seen[sc].update(k for k in ks if k)
    print(f'cache={mode:8s} scope={scope:6s} classifier calls {calls} = {100*calls/N:.1f}% of bash calls')
json.dump([[m['i'],v,[list(o) for o in outs]] for m,v,outs in res], open('tiers_out.json','w'))
top=collections.Counter(o[2] for _,v,outs in res if v=='classify' for o in outs if o[0]=='classify' and o[1]=='opaque')
print('top opaque patterns:', top.most_common(25))

# breakdown of the calls under the repo pattern cache + state size
seen=collections.defaultdict(set); why_calls=collections.Counter(); sizes=[]; per_model=collections.Counter(); per_model_all=collections.Counter()
for m,v,outs in res:
    per_model_all[m['model']]+=1
    if v!='classify': continue
    ks=[(key,why) for vv,why,key,t in outs if vv=='classify']
    miss=[(k,w) for k,w in ks if k is None or k not in seen[m['hub']]]
    if miss:
        why_calls['inline code' if any(w=='inline code' for k,w in miss) else miss[0][1].split(' /')[0]]+=1
        seen[m['hub']].update(k for k,w in ks if k); sizes.append(len(m['cmd'])); per_model[m['model']]+=1
sizes.sort()
print('calls by reason', why_calls.most_common(8))
print('cmd chars p50',sizes[len(sizes)//2],'p90',sizes[int(len(sizes)*.9)],'mean',sum(sizes)//len(sizes),'max',sizes[-1])
print('models', [(k,per_model[k],per_model_all[k]) for k in per_model_all])
