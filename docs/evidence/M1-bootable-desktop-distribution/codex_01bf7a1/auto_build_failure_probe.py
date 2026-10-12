#!/usr/bin/env python3
"""Reviewer-only orchestration probe. Mocked lb/apt; no image or authentication proof."""
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

ForeignNames = ("zzz_cham3_kali.list", "zzz_cham3_librewolf.list", "cham3_kali.pref", "cham3_librewolf.pref")


def probe(repo, fail_stage):
    with tempfile.TemporaryDirectory(prefix="cham3-review-auto-build-") as scratch:
        work = Path(scratch)
        archives = work / "config/archives"
        archives.mkdir(parents=True)
        for name in ForeignNames:
            (archives / name).write_text(name + "\n")
        source_dir = work / "chroot/etc/apt"
        source_dir.mkdir(parents=True)
        (source_dir / "sources.list").write_text(
            "deb [trusted=yes] file:/run/live/medium trixie main\n"
            "deb http://deb.debian.org/debian trixie main\n"
        )
        lists = work / "chroot/var/lib/apt/lists"
        lists.mkdir(parents=True)
        (lists / "_run_live_medium_Packages").write_text("unsigned index\n")
        (lists / "deb.debian.org_Packages").write_text("keep\n")
        bin_dir = work / "bin"
        bin_dir.mkdir()
        (bin_dir / "lb").write_text(
            '#!/bin/sh\nset -eu\nprintf "%s\\n" "$1" >> stages.log\n'
            'if [ "$1" = installer ]; then\n'
            '  for name in zzz_cham3_kali.list zzz_cham3_librewolf.list cham3_kali.pref cham3_librewolf.pref; do\n'
            '    [ ! -e "config/archives/$name" ] || exit 99\n'
            '  done\nfi\n'
            'if [ "$1" = "$PROBE_FAIL_STAGE" ]; then exit 23; fi\n'
        )
        (bin_dir / "apt-get").write_text(
            '#!/bin/sh\nset -eu\n'
            'case " $* " in *" download apt-mirror-setup "*) '
            'printf "mock udeb\\n" > apt-mirror-setup_1_all.udeb ;; esac\n'
        )
        for path in bin_dir.iterdir():
            path.chmod(0o755)
        shutil.copy(repo / "build/live-desktop/auto/build", work / "auto_build")
        env = dict(os.environ, PATH=str(bin_dir) + os.pathsep + os.environ["PATH"],
                   PROBE_FAIL_STAGE=fail_stage)
        result = subprocess.run(["/bin/sh", "auto_build"], cwd=work, env=env,
                                capture_output=True, text=True, timeout=30)
        stages = (work / "stages.log").read_text().splitlines()
        assert all((archives / name).read_text() == name + "\n" for name in ForeignNames)
        assert not (work / ".cham3_installer_stash").exists()
        if fail_stage:
            assert result.returncode == 23, (result.returncode, result.stderr)
            assert stages[-1] == fail_stage, stages
            assert stages == ["bootstrap", "chroot", "installer", "binary", "source"][:stages.index(fail_stage) + 1]
        else:
            assert result.returncode == 0, result.stderr
            assert stages == ["bootstrap", "chroot", "installer", "binary", "source"]
            assert "trusted=yes" not in (source_dir / "sources.list").read_text()
            assert not (lists / "_run_live_medium_Packages").exists()
            assert (lists / "deb.debian.org_Packages").read_text() == "keep\n"
        print("PASS mocked auto/build stage=%s rc=%d stages=%s foreign files restored" %
              (fail_stage or "success", result.returncode, ",".join(stages)))


if __name__ == "__main__":
    for stage in ("bootstrap", "chroot", "installer", "binary", "source", ""):
        probe(Path(sys.argv[1]).resolve(), stage)
