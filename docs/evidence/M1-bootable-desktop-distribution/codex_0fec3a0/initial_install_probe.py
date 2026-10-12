"""Real dpkg ordering, exact candidate scripts in a dependency-free harness, no KDE install."""
import sys, os, subprocess, tempfile, glob, json, stat
sys.path.insert(0,'/workspace/M1-rereview/tests/container')
import test_m1_container_capture as h
os.environ['DEBIAN_FRONTEND']='noninteractive'
scratch=tempfile.mkdtemp(prefix='codex-initial-',dir='/tmp')
def run(argv,check=True):
 r=subprocess.run(argv,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
 print('COMMAND',argv,'EXIT',r.returncode,flush=True)
 if check and r.returncode: print(r.stdout,flush=True);raise RuntimeError('command failed')
 return r
run(['dpkg','--purge','cham3-desktop'],check=False)
run(['apt-get','download','wireshark-common'])
deb=glob.glob('/workspace/M1-rereview/wireshark-common_*.deb')[0]
run(['apt-get','purge','-y','wireshark-common'])
subprocess.run(['debconf-set-selections'],input='wireshark-common wireshark-common/install-setuid boolean true\n',text=True,check=True)
stub=h.dependency_free_stub(glob.glob('/workspace/M1-rereview/out/packages/cham3-desktop_*.deb')[0],scratch)
run(['dpkg','--unpack',stub,deb])
cap='/usr/sbin/setcap';os.rename(cap,cap+'.codex-real')
try:
 open(cap,'w').write('#!/bin/sh\necho "Codex injected setcap failure" >&2\nexit 1\n');os.chmod(cap,0o755)
 run(['dpkg','--configure','wireshark-common'])
 print('BEFORE_CHAM3_CONFIGURE',json.dumps(h.dumpcap_state()),flush=True)
 print(run(['dpkg-query','-W','-f=${Package} ${Status}\n','cham3-desktop','wireshark-common']).stdout,flush=True)
 print('OVERRIDE_BEFORE',run(['dpkg-statoverride','--list','/usr/bin/dumpcap'],False).stdout,flush=True)
 run(['dpkg','--configure','cham3-desktop'])
 print('AFTER_CHAM3_CONFIGURE',json.dumps(h.dumpcap_state()),flush=True)
finally:
 os.unlink(cap);os.rename(cap+'.codex-real',cap)
 run(['dpkg-reconfigure','-f','noninteractive','cham3-desktop'])
 os.unlink(deb)
