import sys, os, tempfile, subprocess,json
sys.path.insert(0,'/workspace/M1-rereview/tests/container')
import test_m1_container_capture as h
for argv in [['/usr/bin/dumpcap','-D'],['sh','-c','exec /usr/bin/dumpcap -D']]:
 r=subprocess.run(h.setpriv_prefix(h.NonmemberUser)+argv,capture_output=True,text=True)
 print(json.dumps({'argv':h.setpriv_prefix(h.NonmemberUser)+argv,'rc':r.returncode,'stdout':r.stdout,'stderr':r.stderr}))
s=tempfile.mkdtemp(prefix='codex-capture-harness-')
print('root scratch mode',oct(os.stat(s).st_mode & 0o777), 'owner',os.stat(s).st_uid)
print('existing harness member output',h.capture_as_member(s,'diagnostic'))
