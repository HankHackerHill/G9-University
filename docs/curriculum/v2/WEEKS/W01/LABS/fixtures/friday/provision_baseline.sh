#!/bin/bash
# Provision the clean week 1 Friday baseline on G9-TGT-01. Synthetic. It plants no fault.
# Run as root inside the guest only:   bash provision_baseline.sh <fixtures-root>
# Faults are applied afterwards by the preparer, from the instructor catalogue, and never by this file.
# It refuses to run unless /etc/g9-lab-guest names G9-TGT-01.
set -eu

FIX="${1:?usage: bash provision_baseline.sh <fixtures-root>}"

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
for f in guided/g9toy_reportd.py friday/g9toy-spool.conf friday/g9toy-spool.service \
         friday/g9toy-spool.service.d/90-sandbox.conf friday/sshd_config.d/10-g9-lab.conf friday/outbox/batch_0001.txt; do
  [ -f "$FIX/$f" ] || { echo "refusing: missing $FIX/$f" >&2; exit 1; }
done

groupadd --force shippers
groupadd --force loaders
id svcspool >/dev/null 2>&1 || useradd --system --user-group --no-create-home --shell /usr/sbin/nologin svcspool
id labanalyst >/dev/null 2>&1 || useradd --create-home --shell /bin/bash labanalyst
id labuser6 >/dev/null 2>&1 || useradd --create-home --shell /bin/bash --groups shippers labuser6
id labuser7 >/dev/null 2>&1 || useradd --create-home --shell /bin/bash --groups loaders labuser7

install -d -m 0755 -o root -g root /opt/g9toy /etc/g9toy-spool /srv/g9toy-spool
install -m 0644 -o root -g root "$FIX/guided/g9toy_reportd.py" /opt/g9toy/g9toy_reportd.py
install -m 0644 -o root -g root "$FIX/friday/g9toy-spool.conf" /etc/g9toy-spool/g9toy.conf
install -m 0644 -o root -g root "$FIX/friday/g9toy-spool.service" /etc/systemd/system/g9toy-spool.service
install -d -m 0755 -o root -g root /etc/systemd/system/g9toy-spool.service.d
install -m 0644 -o root -g root "$FIX/friday/g9toy-spool.service.d/90-sandbox.conf" /etc/systemd/system/g9toy-spool.service.d/90-sandbox.conf

install -d -m 0750 -o svcspool -g shippers /srv/g9toy-spool/outbox
chmod 2750 /srv/g9toy-spool/outbox
install -m 0640 -o svcspool -g shippers "$FIX/friday/outbox/batch_0001.txt" /srv/g9toy-spool/outbox/batch_0001.txt
install -d -m 0730 -o svcspool -g loaders /srv/g9toy-spool/inbox
chmod 2730 /srv/g9toy-spool/inbox
install -d -m 0750 -o svcspool -g svcspool /srv/g9toy-spool/scratch

install -d -m 0755 -o root -g root /etc/ssh/sshd_config.d
install -m 0644 -o root -g root "$FIX/friday/sshd_config.d/10-g9-lab.conf" /etc/ssh/sshd_config.d/10-g9-lab.conf
sshd -t
systemctl reload ssh.service || systemctl restart ssh.service

systemctl daemon-reload
systemctl enable --now g9toy-spool.service
printf 'friday baseline provisioned %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > /etc/g9-lab-state
echo "baseline provisioned; record the time written in /etc/g9-lab-state"
