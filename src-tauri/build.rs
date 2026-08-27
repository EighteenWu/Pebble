fn main() {
    emit_android_page_size_link_args();
    for key in [
        "GOOGLE_CLIENT_ID",
        "GOOGLE_CLIENT_SECRET",
        "MICROSOFT_CLIENT_ID",
        "MICROSOFT_CLIENT_SECRET",
    ] {
        emit_env_from_dotenv("../.env", key);
    }
    tauri_build::build()
}

/// Android 15 / Snapdragon 8 Elite devices use 16 KB pages. NDK 27 still
/// emits 4 KB ELF LOAD alignment unless the linker is told otherwise.
/// Unaligned Rust `.so` files crash in `dlopen` with no Java dialog.
fn emit_android_page_size_link_args() {
    let flags = android_page_size_link_args();
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("android") {
        return;
    }
    println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_OS");
    for flag in flags {
        println!("cargo:rustc-link-arg={flag}");
    }
}

fn android_page_size_link_args() -> &'static [&'static str] {
    &[
        "-Wl,-z,max-page-size=16384",
        "-Wl,-z,common-page-size=16384",
    ]
}

fn emit_env_from_dotenv(path: &str, key: &str) {
    println!("cargo:rerun-if-changed={path}");
    println!("cargo:rerun-if-env-changed={key}");
    if let Ok(value) = std::env::var(key) {
        println!("cargo:rustc-env={key}={value}");
        return;
    }

    let Ok(contents) = std::fs::read_to_string(path) else {
        return;
    };
    let Some(value) = dotenv_lookup_from_str(&contents, key) else {
        return;
    };
    println!("cargo:rustc-env={key}={value}");
}

fn dotenv_lookup_from_str(contents: &str, key: &str) -> Option<String> {
    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        if name.trim() != key {
            continue;
        }
        let value = value.trim();
        let unquoted = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
            .unwrap_or(value);
        return Some(unquoted.to_string());
    }
    None
}
