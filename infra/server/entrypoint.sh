#!/bin/sh
set -eu

AUTHORIZED_KEY_FILE="${WARA_TEST_SERVER_AUTHORIZED_KEY_FILE:-/tmp/wara-authorized-keys/id_ed25519.pub}"
REMOTE_ROOT="${WARA_TEST_SERVER_REMOTE_ROOT:-/tmp/wara-docker-server/services}"

mkdir -p /run/sshd /home/deploy/.ssh "${REMOTE_ROOT}"
ssh-keygen -A >/dev/null

if [ -n "${WARA_TEST_SERVER_AUTHORIZED_KEY:-}" ]; then
    printf '%s\n' "${WARA_TEST_SERVER_AUTHORIZED_KEY}" > /home/deploy/.ssh/authorized_keys
elif [ -f "${AUTHORIZED_KEY_FILE}" ]; then
    cp "${AUTHORIZED_KEY_FILE}" /home/deploy/.ssh/authorized_keys
else
    echo "missing SSH public key: set WARA_TEST_SERVER_AUTHORIZED_KEY or mount ${AUTHORIZED_KEY_FILE}" >&2
    exit 1
fi

chmod 700 /home/deploy/.ssh
chmod 600 /home/deploy/.ssh/authorized_keys
chown -R deploy:deploy /home/deploy "${REMOTE_ROOT}"

if [ -S /var/run/docker.sock ]; then
    DOCKER_SOCKET_GID="$(stat -c '%g' /var/run/docker.sock)"
    if ! getent group "${DOCKER_SOCKET_GID}" >/dev/null 2>&1; then
        addgroup -g "${DOCKER_SOCKET_GID}" dockerhost >/dev/null 2>&1 || true
    fi
    DOCKER_SOCKET_GROUP="$(getent group "${DOCKER_SOCKET_GID}" | cut -d: -f1 || true)"
    if [ -n "${DOCKER_SOCKET_GROUP}" ]; then
        addgroup deploy "${DOCKER_SOCKET_GROUP}" >/dev/null 2>&1 || true
    fi
fi

cat >/etc/ssh/sshd_config <<'EOF'
Port 22
PermitRootLogin no
PasswordAuthentication no
KbdInteractiveAuthentication no
PubkeyAuthentication yes
AuthorizedKeysFile .ssh/authorized_keys
AllowUsers deploy
X11Forwarding no
AllowTcpForwarding no
PermitTunnel no
PrintMotd no
Subsystem sftp internal-sftp
EOF

exec /usr/sbin/sshd -D -e
