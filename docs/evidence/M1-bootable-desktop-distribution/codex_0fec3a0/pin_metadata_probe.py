"""Synthetic signed repositories; no network, no installation, private disposable APT roots."""
from pathlib import Path
import tempfile, subprocess, hashlib, json, os
base=Path(tempfile.mkdtemp(prefix='codex-pin-',dir='/tmp'));base.chmod(0o755)
gpg=base/'gnupg';gpg.mkdir(mode=0o700)
def run(argv):
 r=subprocess.run(argv,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
 if r.returncode: raise RuntimeError(' '.join(argv)+'\n'+r.stdout)
 return r.stdout
run(['gpg','--homedir',str(gpg),'--batch','--pinentry-mode','loopback','--passphrase','','--quick-generate-key','Codex Disposable Pin Test','rsa2048','sign','1d'])
key=base/'test-key.gpg';key.write_bytes(subprocess.check_output(['gpg','--homedir',str(gpg),'--export']));key.chmod(0o644)
repo_root=Path('/workspace/M1-rereview')
for label,expected_origin,actual_origin,suite,actual_codename,pref in [
 ('kali_positive','Kali','Kali','kali-rolling','kali-rolling','cham3_kali.pref'),
 ('kali_changed','Kali','KaliNew','kali-rolling','kali-rolling','cham3_kali.pref'),
 ('librewolf_positive','LibreWolf','LibreWolf','librewolf','librewolf','cham3_librewolf.pref'),
 ('librewolf_changed','LibreWolf','LibreWolf','librewolf','librewolf-new','cham3_librewolf.pref')]:
 repo=base/label/'repo';d=repo/'dists'/suite;pkgs=d/'main/binary-amd64/Packages';pkgs.parent.mkdir(parents=True)
 pkgs.write_text('Package: codex-unlisted\nVersion: 9.0\nArchitecture: amd64\nMaintainer: Codex test <test@example.invalid>\nDescription: inert test package\nFilename: pool/test.deb\nSize: 100\nSHA256: '+'0'*64+'\n\n')
 data=pkgs.read_bytes();rel=d/'Release'
 rel.write_text(f'Date: Mon, 12 Oct 2026 02:30:00 UTC\nOrigin: {actual_origin}\nLabel: Synthetic\nSuite: {suite}\nCodename: {actual_codename}\nArchitectures: amd64\nComponents: main\nSHA256:\n {hashlib.sha256(data).hexdigest()} {len(data)} main/binary-amd64/Packages\n')
 run(['gpg','--homedir',str(gpg),'--batch','--yes','--clearsign','--output',str(d/'InRelease'),str(rel)])
 root=base/label/'apt';(root/'etc/apt/sources.list.d').mkdir(parents=True);(root/'etc/apt/preferences.d').mkdir();(root/'etc/apt/trusted.gpg.d').mkdir()
 (root/'var/lib/apt/lists/partial').mkdir(parents=True);(root/'var/cache/apt/archives/partial').mkdir(parents=True);(root/'var/lib/dpkg').mkdir(parents=True);(root/'var/lib/dpkg/status').touch()
 (root/'etc/apt/sources.list').write_text(f'deb [signed-by={key}] file:{repo} {suite} main\n')
 (root/'etc/apt/preferences.d/policy').write_text((repo_root/'build/live-desktop/config/archives'/pref).read_text())
 opts=['-o',f'Dir={root}','-o',f'Dir::State::status={root}/var/lib/dpkg/status','-o','APT::Architecture=amd64','-o','APT::Get::List-Cleanup=false','-o','Acquire::Languages=none','-o','APT::Sandbox::User=root']
 upd=run(['apt-get']+opts+['update']);policy=run(['apt-cache']+opts+['policy','codex-unlisted'])
 print(json.dumps({'label':label,'origin':actual_origin,'codename':actual_codename,'signed_update':upd,'policy':policy}),flush=True)
print('Synthetic apt roots:',base)
