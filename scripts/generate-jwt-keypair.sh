#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/generate-jwt-keypair.sh [key-id] [output-dir]

Generates an RSA JWT signing keypair and writes:
  private.pem          Secret signing key. Store in deployment secrets.
  public.pem           Public verification key.
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

key_id="${1:-jwt-$(date -u +%Y%m%d%H%M%S)}"
output_dir="${2:-jwt-key-${key_id}}"

case "$key_id" in
  *[!A-Za-z0-9._-]* | "")
    echo "key-id must contain only letters, numbers, dot, underscore, or dash" >&2
    exit 1
    ;;
esac

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

python3 - "$key_id" "$private_key_path" "$public_key_path" "$env_snippet_path" "$config_snippet_path" <<'PY'
import json
import pathlib
import shlex
import sys

key_id, private_path, public_path, env_path, config_path = sys.argv[1:]
private_key = pathlib.Path(private_path).read_text()
public_key = pathlib.Path(public_path).read_text()

public_keys = json.dumps([{"id": key_id, "public_key_pem": public_key}], separators=(",", ":"))
env_contents = "\n".join(
    [
        f"WARA_JWT_ACTIVE_KEY_ID={shlex.quote(key_id)}",
        f"WARA_JWT_PRIVATE_KEY_PEM={shlex.quote(private_key)}",
        f"WARA_JWT_PUBLIC_KEYS={shlex.quote(public_keys)}",
        "",
    ]
)
pathlib.Path(env_path).write_text(env_contents)

def yaml_block(value: str, indent: int = 2) -> str:
    prefix = " " * indent
    return "".join(f"{prefix}{line}\n" for line in value.splitlines())

config_contents = (
    f"jwt_active_key_id: {json.dumps(key_id)}\n"
    "jwt_private_key_pem: |\n"
    f"{yaml_block(private_key)}"
    "jwt_public_keys:\n"
    f"  - id: {json.dumps(key_id)}\n"
    "    public_key_pem: |\n"
    f"{yaml_block(public_key, 6)}"
)
pathlib.Path(config_path).write_text(config_contents)
PY

chmod 600 "$private_key_path" "$public_key_path" "$env_snippet_path" "$config_snippet_path"

cat <<EOF
Generated JWT keypair for key id: $key_id

Secret files:
  $private_key_path
  $env_snippet_path
  $config_snippet_path

Public key:
  $public_key_path

Store the private key and snippets in your deployment secret manager.
EOF
