import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("desktop-only Rust gating", () => {
  it("keeps tray setup behind cfg(desktop)", () => {
    const lib = readFileSync(resolve(process.cwd(), "src-tauri", "src", "lib.rs"), "utf8");
    expect(lib).toMatch(/#\[cfg\(desktop\)\]\s+fn setup_tray/);
    expect(lib).toMatch(/#\[cfg\(desktop\)\]\s+if let Err\(e\) = setup_tray/);
  });

  it("rejects desktop OAuth localhost callbacks on mobile", () => {
    const oauth = readFileSync(
      resolve(process.cwd(), "src-tauri", "src", "commands", "oauth.rs"),
      "utf8",
    );
    expect(oauth).toContain("oauth_unavailable_on_mobile_message");
    expect(oauth).toContain("#[cfg(not(desktop))]");
    expect(oauth).toContain("complete_desktop_oauth_flow");
  });

  it("does not start IMAP IDLE on Android", () => {
    const sync = readFileSync(resolve(process.cwd(), "crates", "pebble-mail", "src", "sync.rs"), "utf8");
    expect(sync).toContain('cfg!(target_os = "android")');
    expect(sync).toContain("open-app / polling sync only");
  });

  it("keeps core mail and store crates available on Android", () => {
    const cargo = readFileSync(resolve(process.cwd(), "src-tauri", "Cargo.toml"), "utf8");
    const [shared] = cargo.split("[target.'cfg(not(any(target_os = \"android\", target_os = \"ios\")))'.dependencies]");
    expect(shared).toContain("pebble-mail");
    expect(shared).toContain("pebble-store");
    expect(shared).toContain("pebble-crypto");
    expect(cargo).toContain("features = [\"tray-icon\"]");
    expect(cargo).toContain("opener = \"0.7\"");
  });
});
