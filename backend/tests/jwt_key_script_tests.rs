use std::{fs, process::Command};

use jsonwebtoken::EncodingKey;
use uuid::Uuid;

#[test]
fn jwt_keypair_helper_generates_snippets_without_printing_secrets() {
    let output_dir = std::env::temp_dir().join(format!("wara-jwt-key-{}", Uuid::now_v7()));
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("backend crate should have a repository parent")
        .join("scripts/generate-jwt-keypair.sh");

    let output = Command::new(&script)
        .arg(&output_dir)
        .output()
        .expect("run JWT keypair helper");

    assert!(
        output.status.success(),
        "helper failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("BEGIN PRIVATE KEY"));
    assert!(!stdout.contains("BEGIN RSA PRIVATE KEY"));
    assert!(stdout.contains("Generated JWT keypair."));

    let private_key = fs::read_to_string(output_dir.join("private.pem")).expect("private key");
    let public_key = fs::read_to_string(output_dir.join("public.pem")).expect("public key");
    let env_snippet = fs::read_to_string(output_dir.join("env.snippet")).expect("env snippet");
    let config_snippet =
        fs::read_to_string(output_dir.join("config.snippet.yml")).expect("config snippet");

    assert!(private_key.contains("BEGIN") && private_key.contains("PRIVATE KEY"));
    EncodingKey::from_rsa_pem(private_key.as_bytes()).expect("private key should sign JWTs");
    assert!(public_key.contains("BEGIN PUBLIC KEY"));
    assert!(env_snippet.contains("WARA_JWT_PRIVATE_KEY_PEM="));
    assert!(env_snippet.contains("WARA_JWT_PUBLIC_KEY="));
    assert!(config_snippet.contains("jwt_public_key:"));

    let _ = fs::remove_dir_all(output_dir);
}
