from pathlib import Path
import subprocess, os
src = Path('/tmp/review-control-wireshark-common/postinst').read_text()
root = Path('/tmp/codex-capture-probe'); root.mkdir(exist_ok=True)
program = root/'dumpcap'; program.write_text('disposable non-executable test content')
src = src.replace('. /usr/share/debconf/confmodule', 'db_get() { RET=true; }\ndb_input() { :; }\ndb_go() { :; }')
src = src.replace('PROGRAM=$(dpkg-divert --truename /usr/bin/dumpcap)', 'PROGRAM=/tmp/codex-capture-probe/dumpcap')
script = root/'postinst'; script.write_text(src)
for case in ['capability_success', 'capability_failure']:
 subprocess.run(['setcap','-r',str(program)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
 program.chmod(0o644)
 env = os.environ.copy()
 if case == 'capability_failure':
  stub = root/'setcap'; stub.write_text('#!/bin/sh\nexit 1\n'); stub.chmod(0o755)
  env['PATH'] = str(root)+':'+env['PATH']
 result = subprocess.run(['sh',str(script),'configure'],env=env)
 print(case, 'exit', result.returncode, 'mode', oct(program.stat().st_mode & 0o7777), flush=True)
 subprocess.run(['getcap',str(program)])
