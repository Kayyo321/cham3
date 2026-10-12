import subprocess, socket, time, os, json
pfx=['setpriv','--reuid=2001','--regid=2001','--groups=wireshark','sh','-c']
cmd='exec /usr/bin/dumpcap -p -i lo -a duration:4 -c 1 -w /tmp/codex-member-real.pcapng'
p=subprocess.Popen(pfx+[cmd],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
s=socket.socket(socket.AF_INET,socket.SOCK_DGRAM)
while p.poll() is None:
 s.sendto(b'Codex F3 native independent loopback packet',( '127.0.0.1',9));time.sleep(0.2)
a,b=p.communicate()
print(json.dumps({'command':pfx+[cmd],'exit':p.returncode,'stdout':a,'stderr':b,'bytes':os.stat('/tmp/codex-member-real.pcapng').st_size}))
r=subprocess.run(['setpriv','--reuid=2002','--regid=2002','--clear-groups','sh','-c','exec /usr/bin/dumpcap -D'],capture_output=True,text=True)
print(json.dumps({'nonmember_exit':r.returncode,'stderr':r.stderr}))
