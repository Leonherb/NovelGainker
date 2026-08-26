use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

fn required_text<'a>(value: &'a Value, key: &str) -> &'a str {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("Provider Preset field {key} must be a string"))
}

fn main() {
    const MANIFEST: &str = "provider-presets.v1.json";
    println!("cargo:rerun-if-changed={MANIFEST}");

    let bytes = fs::read(Path::new(MANIFEST)).expect("Provider Preset v1 must be readable");
    assert!(
        !bytes.starts_with(&[0xef, 0xbb, 0xbf]),
        "Provider Preset must not use BOM"
    );
    let root: Value =
        serde_json::from_slice(&bytes).expect("Provider Preset v1 must be valid JSON");
    assert_eq!(root.get("presetVersion").and_then(Value::as_u64), Some(1));

    let providers = root
        .get("providers")
        .and_then(Value::as_array)
        .expect("Provider Preset providers must be an array");
    assert_eq!(
        providers.len(),
        2,
        "Provider Preset v1 must contain exactly two providers"
    );

    let mut ids = BTreeSet::new();
    for provider in providers {
        let id = required_text(provider, "id");
        assert!(ids.insert(id), "Provider IDs must be unique");
        let (base_url, endpoint, protocol, model) = match id {
            "deepseek" => (
                "https://api.deepseek.com",
                "/chat/completions",
                "deepseek-chat",
                "deepseek-v4-pro",
            ),
            "openai" => (
                "https://api.openai.com/v1",
                "/responses",
                "openai-responses",
                "gpt-5.6-terra",
            ),
            _ => panic!("Provider Preset contains an unapproved Provider"),
        };
        assert_eq!(required_text(provider, "baseUrl"), base_url);
        assert_eq!(required_text(provider, "endpoint"), endpoint);
        assert_eq!(required_text(provider, "protocol"), protocol);
        assert_eq!(required_text(provider, "model"), model);
        assert_eq!(
            provider.get("timeoutSeconds").and_then(Value::as_u64),
            Some(120)
        );
        assert!(base_url.starts_with("https://"));
        assert!(!base_url.contains("localhost"));
        assert!(!base_url.contains("127.0.0.1"));
    }
    assert_eq!(ids, BTreeSet::from(["deepseek", "openai"]));

    let hash = Sha256::digest(&bytes);
    println!("cargo:rustc-env=NOVELGAINKER_PROVIDER_PRESET_SHA256={hash:x}");
}
