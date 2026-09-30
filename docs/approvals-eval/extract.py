import json,glob,os,sys
out=open('calls.jsonl','w')
n=0
for f in glob.glob(os.path.expanduser('~/.bise/hubs/*/agents/*/wire.log*')):
    hub=f.split('/hubs/')[1].split('/')[0]; agent=f.split('/agents/')[1].split('/')[0]
    with open(f,errors='replace') as fh:
        for line in fh:
            if not line.startswith('  ev: {"type":"assistant_message"'): continue
            try: d=json.loads(line[6:])
            except Exception: continue
            model=d['data'].get('model','')
            for c in d['data'].get('calls',[]):
                try: a=json.loads(c['args'])
                except Exception: a={'_raw':c['args']}
                out.write(json.dumps({'hub':hub,'agent':agent,'model':model,'name':c['name'],'args':a})+'\n'); n+=1
print(n)
