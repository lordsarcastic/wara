#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/generate-jwt-keypair.sh [output-dir]

Generates an RSA JWT signing keypair and writes:
  private.pem          Secret signing key. Store in deployment secrets.
  public.pem           Public verification key used only as seed material.
  env.snippet          Environment-variable snippet for secret managers.
  config.snippet.yml   ~/.wara/config.yml snippet.

The script does not print private key material to stdout.
USAGE
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

command -v openssl >/dev/null 2>&1 || {
  echo "openssl is required" >&2
  exit 1
}
command -v python3 >/dev/null 2>&1 || {
  echo "python3 is required to safely quote config snippets" >&2
  exit 1
}

default_output_dir="$(
  python3 - <<'PY'
import os
import random
import time
import uuid

timestamp_ms = int(time.time() * 1000) & ((1 << 48) - 1)
random_bits = int.from_bytes(os.urandom(10), "big") & ((1 << 74) - 1)
value = (timestamp_ms << 80) | (0x7 << 76) | random_bits
value &= ~(0b11 << 62)
value |= 0b10 << 62
print(uuid.UUID(int=value))
PY
)"
output_dir="${1:-jwt-key-${default_output_dir}}"

umask 077
mkdir -p "$output_dir"
chmod 700 "$output_dir"

private_key_path="$output_dir/private.pem"
public_key_path="$output_dir/public.pem"
env_snippet_path="$output_dir/env.snippet"
config_snippet_path="$output_dir/config.snippet.yml"

if [[ -e "$private_key_path" || -e "$public_key_path" ]]; then
  echo "refusing to overwrite existing key files in $output_dir" >&2
  exit 1
fi

openssl genrsa -out "$private_key_path" 2048 >/dev/null 2>&1
openssl rsa -in "$private_key_path" -pubout -out "$public_key_path" >/dev/null 2>&1

python3 - "$private_key_path" "$public_key_path" "$env_snippet_path" "$config_snippet_path" <<'PY'
import pathlib
import shlex
import sys

private_path, public_path, env_path, config_path = sys.argv[1:]
private_key = pathlib.Path(private_path).read_text()
public_key = pathlib.Path(public_path).read_text()

env_contents = "\n".join(
    [
        f"WARA_JWT_PRIVATE_KEY_PEM={shlex.quote(private_key)}",
        f"WARA_JWT_PUBLIC_KEY={shlex.quote(public_key)}",
        "",
    ]
)
pathlib.Path(env_path).write_text(env_contents)

def yaml_block(value: str, indent: int = 2) -> str:
    prefix = " " * indent
    return "".join(f"{prefix}{line}\n" for line in value.splitlines())

config_contents = (
    "jwt_private_key_pem: |\n"
    f"{yaml_block(private_key)}"
    "jwt_public_key: |\n"
    f"{yaml_block(public_key)}"
)
pathlib.Path(config_path).write_text(config_contents)
PY

chmod 600 "$private_key_path" "$public_key_path" "$env_snippet_path" "$config_snippet_path"

cat <<EOF
Generated JWT keypair.

Secret files:
  $private_key_path
  $env_snippet_path
  $config_snippet_path

Public key:
  $public_key_path

Store the private key and snippets in your deployment secret manager.
The backend stores the public key as JWK components, not PEM, at startup.
EOF
