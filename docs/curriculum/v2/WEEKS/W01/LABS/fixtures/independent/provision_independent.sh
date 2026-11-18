#!/bin/bash
# Provision the week 1 independent-lab state on G9-TGT-01. Synthetic.
# Run as root inside the guest only:   bash provision_independent.sh <fixtures-root>
# It creates accounts, directories and service files, replaces the guest's sshd_config
# (the original is kept as /root/g9-sshd_config.orig) and starts the service.
# It refuses to run unless /etc/g9-lab-guest names G9-TGT-01.
set -eu

FIX="${1:?usage: bash provision_independent.sh <fixtures-root>}"

if [ "$(id -u)" -ne 0 ]; then
  echo "refusing: run as root inside the guest" >&2
  exit 1
fi
if ! grep -qx 'G9-TGT-01' /etc/g9-lab-guest 2>/dev/null; then
  echo "refusing: /etc/g9-lab-guest does not name G9-TGT-01" >&2
  exit 1
fi
if ! command -v sshd >/dev/null 2>&1; then
  echo "refusing: no sshd in this guest; do not install it, use the paper variant" >&2
  exit 1
fi
for f in guided/g9toy_reportd.py independent/g9toy-audit.conf independent/g9toy-audit.service \
         independent/sudoers.d/40-ops independent/sshd_config independent/sshd_config.d/10-image.conf \
         independent/sshd_config.d/60-hardening.conf independent/spool/rec_0001.txt independent/spool/rec_0002.txt; do
  [ -f "$FIX/$f" ] || { echo "refusing: missing $FIX/$f" >&2; exit 1; }
done
visudo -cf "$FIX/independent/sudoers.d/40-ops"

groupadd --force auditors
groupadd --force ops
id svcaudit >/dev/null 2>&1 || useradd --system --user-group --no-create-home --shell /usr/sbin/nologin svcaudit
id labuser3 >/dev/null 2>&1 || useradd --create-home --shell /bin/bash --groups auditors labuser3
id labuser4 >/dev/null 2>&1 || useradd --create-home --shell /bin/bash --groups ops labuser4

install -d -m 0755 -o root -g root /opt/g9toy /etc/g9toy-audit /srv/g9toy-audit
install -m 0644 -o root -g root "$FIX/guided/g9toy_reportd.py" /opt/g9toy/g9toy_reportd.py
install -m 0644 -o root -g root "$FIX/independent/g9toy-audit.conf" /etc/g9toy-audit/g9toy.conf
install -m 0644 -o root -g root "$FIX/independent/g9toy-audit.service" /etc/systemd/system/g9toy-audit.service

install -d -m 0770 -o svcaudit -g ops /srv/g9toy-audit/spool
chmod 2770 /srv/g9toy-audit/spool
install -m 0660 -o svcaudit -g ops "$FIX/independent/spool/rec_0001.txt" /srv/g9toy-audit/spool/rec_0001.txt
install -m 0660 -o svcaudit -g ops "$FIX/independent/spool/rec_0002.txt" /srv/g9toy-audit/spool/rec_0002.txt
install -d -m 0750 -o svcaudit -g svcaudit /srv/g9toy-audit/scratch
install -d -m 0700 -o root -g root /srv/g9toy-audit/restricted
printf 'synthetic restricted marker\n' > /srv/g9toy-audit/restricted/intended_marker.txt
chmod 0600 /srv/g9toy-audit/restricted/intended_marker.txt

install -m 0440 -o root -g root "$FIX/independent/sudoers.d/40-ops" /etc/sudoers.d/40-ops

[ -f /root/g9-sshd_config.orig ] || cp -p /etc/ssh/sshd_config /root/g9-sshd_config.orig
install -d -m 0755 -o root -g root /etc/ssh/sshd_config.d
install -m 0644 -o root -g root "$FIX/independent/sshd_config" /etc/ssh/sshd_config
install -m 0644 -o root -g root "$FIX/independent/sshd_config.d/10-image.conf" /etc/ssh/sshd_config.d/10-image.conf
install -m 0644 -o root -g root "$FIX/independent/sshd_config.d/60-hardening.conf" /etc/ssh/sshd_config.d/60-hardening.conf
sshd -t
systemctl reload ssh.service || systemctl restart ssh.service

systemctl daemon-reload
systemctl enable --now g9toy-audit.service
printf 'independent state provisioned %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > /etc/g9-lab-state
echo "provisioned; record the time written in /etc/g9-lab-state"
