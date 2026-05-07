use std::fs;
use std::path::Path;

fn main() {
    // Tell Cargo to re-run this script if .env changes
    println!("cargo:rerun-if-changed=.env");
    println!("cargo:rerun-if-env-changed=GEMINI_API_KEY");

    // Priority 1: shell environment variable (CI / developer override)
    if let Ok(key) = std::env::var("GEMINI_API_KEY") {
        if !key.trim().is_empty() {
            println!("cargo:rustc-env=GEMINI_API_KEY={}", key.trim());
            tauri_build::build();
            return;
        }
    }

    // Priority 2: read from .env file in the src-tauri directory
    for path in &[".env", "../.env"] {
        if let Ok(contents) = fs::read_to_string(Path::new(path)) {
            for line in contents.lines() {
                let line = line.trim();
                if line.starts_with('#') || !line.contains('=') {
                    continue;
                }
                if let Some(rest) = line.strip_prefix("GEMINI_API_KEY=") {
                    let key = rest.trim().trim_matches('"').trim_matches('\'');
                    if !key.is_empty() {
                        println!("cargo:rustc-env=GEMINI_API_KEY={}", key);
                        tauri_build::build();
                        return;
                    }
                }
            }
        }
    }

    // If we get here, no key was found — emit an empty string so env!() compiles,
    // but the binary will fail at runtime with a clear message.
    println!("cargo:rustc-env=GEMINI_API_KEY=");
    tauri_build::build();
}