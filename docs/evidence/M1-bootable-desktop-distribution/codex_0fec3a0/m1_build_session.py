"""Inject only session proxy/CA trust into disposable build chroot. Candidate source is unchanged."""
import os, subprocess, time
from pathlib import Path
root=Path('/workspace/M1-rereview/out/live-build-work/chroot')
env=dict(os.environ,CHAM3_LIVE_DIR='build/live-desktop',CHAM3_IMAGE_NAME='cham3-m1-desktop-amd64')
p=subprocess.Popen(['build/scripts/build_image.sh'],env=env)
ca_configured=False
while p.poll() is None:
 # Wait for this profile's debootstrap ca-certificates input to exist, after old M0 clean.
 if (root/'etc/apt/apt.conf.d').is_dir() and (root/'etc/ssl/certs/ca-certificates.crt').exists():
  if not ca_configured:
   (root/'run').mkdir(exist_ok=True)
   (root/'run/session-ca.crt').write_bytes(Path('/run/session-ca.crt').read_bytes())
   (root/'etc/apt/apt.conf.d/98-review-session').write_text('Acquire::http::Proxy "'+os.environ['HTTP_PROXY']+'";\nAcquire::https::Proxy "'+os.environ['HTTPS_PROXY']+'";\nAcquire::https::CaInfo "/run/session-ca.crt";\n')
   ca_configured=True
   print('Codex: session proxy and CA configured in disposable chroot; TLS verification retained',flush=True)
 time.sleep(.2)
print('Codex: M1 build exit',p.returncode,'CA configured',ca_configured,flush=True)
raise SystemExit(p.returncode)
