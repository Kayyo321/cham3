#!/bin/bash
# Purpose: member / nonmember packet-capture control inside a disposable cham3 VM
# (F3, F8). Copied into the guest by build/vm/run_m1_vm_checks.py and run as root.
# Creates two throwaway accounts: a deliberate member of the "wireshark" group (the
# way an administrator grants capture) and a nonmember. The tested program is
# executed only after the identity is dropped: setpriv starts a shell as the user
# and the shell executes dumpcap. The member writes into a directory it owns, so an
# output-path problem cannot be mistaken for missing capture capability.
# Prints key=value lines; the harness decides PASS/FAIL. Never run on a real system.
set -eu

member=cham3vmmember
nonmember=cham3vmnonmember
for account in "${member}" "${nonmember}"; do
    id -u "${account}" > /dev/null 2>&1 || useradd --create-home "${account}"
done
if getent group wireshark > /dev/null 2>&1; then
    usermod -aG wireshark "${member}"
    capture_gid=$(getent group wireshark | cut -d: -f3)
else
    capture_gid=
fi

out_dir=$(mktemp -d /tmp/cham3-capture-XXXXXX)
chown "${member}:" "${out_dir}"
chmod 0700 "${out_dir}"
member_uid=$(id -u "${member}")
member_gid=$(id -g "${member}")
nonmember_uid=$(id -u "${nonmember}")
nonmember_gid=$(id -g "${nonmember}")
member_groups=${member_gid}
if [ -n "${capture_gid}" ]; then
    member_groups=${capture_gid},${member_gid}
fi

(
    for _ in $(seq 1 60); do
        printf 'cham3 vm capture probe' > /dev/udp/127.0.0.1/9 2> /dev/null || true
        sleep 0.25
    done
) &
sender=$!

# The single-quoted script is evaluated by the user's shell, not here.
# shellcheck disable=SC2016
exec_helper='exec "$0" "$@"'

member_rc=0
setpriv --reuid="${member_uid}" --regid="${member_gid}" --groups="${member_groups}" -- \
    /bin/sh -c "${exec_helper}" /usr/bin/dumpcap -p -i lo -a duration:8 -c 1 -w "${out_dir}/member.pcapng" \
    > "${out_dir}/member.log" 2>&1 || member_rc=$?
kill "${sender}" 2> /dev/null || true
wait "${sender}" 2> /dev/null || true
member_bytes=$(stat -c %s "${out_dir}/member.pcapng" 2> /dev/null || echo 0)

nonmember_rc=0
setpriv --reuid="${nonmember_uid}" --regid="${nonmember_gid}" --clear-groups -- \
    /bin/sh -c "${exec_helper}" /usr/bin/dumpcap -D > "${out_dir}/nonmember.log" 2>&1 || nonmember_rc=$?

echo "member_rc=${member_rc}"
echo "member_bytes=${member_bytes}"
echo "nonmember_rc=${nonmember_rc}"
echo "member_log=$(tr '\n' ' ' < "${out_dir}/member.log")"
echo "nonmember_log=$(tr '\n' ' ' < "${out_dir}/nonmember.log")"
rm -rf "${out_dir}"
