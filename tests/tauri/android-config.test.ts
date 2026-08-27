import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("Tauri Android config", () => {
  const androidConfig = JSON.parse(
    readFileSync(resolve(process.cwd(), "src-tauri", "tauri.android.conf.json"), "utf8"),
  );
  const packageJson = JSON.parse(readFileSync(resolve(process.cwd(), "package.json"), "utf8"));
  const desktopConfig = JSON.parse(
    readFileSync(resolve(process.cwd(), "src-tauri", "tauri.conf.json"), "utf8"),
  );

  it("does not apply desktop minWidth or custom window decorations", () => {
    const mainWindow = androidConfig.app.windows.find((windowConfig: { label?: string }) => {
      return windowConfig.label === "main";
    });

    expect(mainWindow).toBeTruthy();
    expect(mainWindow.minWidth).toBeUndefined();
    expect(mainWindow.minHeight).toBeUndefined();
    expect(mainWindow.decorations).not.toBe(false);
    expect(mainWindow.visible).toBe(true);
  });

  it("keeps desktop window chrome on the desktop config", () => {
    const mainWindow = desktopConfig.app.windows.find((windowConfig: { label?: string }) => {
      return windowConfig.label === "main";
    });

    expect(mainWindow.minWidth).toBe(800);
    expect(mainWindow.decorations).toBe(false);
  });

  it("exposes Android package scripts", () => {
    expect(packageJson.scripts["dev:android"]).toBe("tauri android dev");
    expect(packageJson.scripts["build:android"]).toBe("tauri android build --apk");
  });

  it("commits the generated Android project and Keystore helper", () => {
    const androidRoot = resolve(process.cwd(), "src-tauri", "gen", "android");
    const keystore = resolve(
      androidRoot,
      "app",
      "src",
      "main",
      "java",
      "com",
      "qingj01",
      "pebble",
      "PebbleKeystore.kt",
    );
    const manifest = resolve(androidRoot, "app", "src", "main", "AndroidManifest.xml");
    expect(existsSync(androidRoot)).toBe(true);
    expect(existsSync(keystore)).toBe(true);
    expect(readFileSync(keystore, "utf8")).toContain("AndroidKeyStore");
    expect(readFileSync(manifest, "utf8")).toContain("POST_NOTIFICATIONS");

    const gradle = readFileSync(resolve(androidRoot, "app", "build.gradle.kts"), "utf8");
    expect(gradle).toContain('rootDirRel = "../../../../"');
  });
});
