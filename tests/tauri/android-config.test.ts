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
    const intents = resolve(
      androidRoot,
      "app",
      "src",
      "main",
      "java",
      "com",
      "qingj01",
      "pebble",
      "PebbleIntents.kt",
    );
    expect(existsSync(keystore)).toBe(true);
    expect(existsSync(intents)).toBe(true);
    expect(readFileSync(keystore, "utf8")).toContain("AndroidKeyStore");
    expect(readFileSync(intents, "utf8")).toContain("showStartupError");
    expect(readFileSync(manifest, "utf8")).toContain("POST_NOTIFICATIONS");

    const gradle = readFileSync(resolve(androidRoot, "app", "build.gradle.kts"), "utf8");
    expect(gradle).toContain('rootDirRel = "../../../../"');
    expect(gradle).toContain('debugSymbolLevel = "none"');
    expect(gradle).toContain("useLegacyPackaging = false");
    expect(gradle).toContain('signingConfig = signingConfigs.getByName("debug")');
    expect(gradle).not.toContain("keepDebugSymbols");
    expect(readFileSync(keystore, "utf8")).toContain("getClassLoader");
  });

  it("passes setup-android packages as a space-separated string", () => {
    const workflow = readFileSync(
      resolve(process.cwd(), ".github", "workflows", "android.yml"),
      "utf8",
    );
    const packagesLine = workflow
      .split("\n")
      .find((line) => line.includes("packages:"));

    expect(packagesLine).toBeTruthy();
    expect(packagesLine).toMatch(
      /packages:\s+platform-tools platforms;android-36 build-tools;36\.0\.0 ndk;27\.2\.12479018\s*$/,
    );
    expect(workflow).toContain("tauri android build --apk --target aarch64 --ci");
    expect(workflow).not.toContain("tauri android build --debug");
    expect(workflow).toContain("ANDROID_SUPPORT_FLEXIBLE_PAGE_SIZES=ON");
    expect(workflow).toContain("scripts/verify-android-apk.sh");
    expect(workflow).not.toContain("${{ env.ANDROID_NDK_HOME }}");
  });

  it("aligns Android native libs to 16 KB pages", () => {
    const cargoConfig = readFileSync(resolve(process.cwd(), ".cargo", "config.toml"), "utf8");
    const buildRs = readFileSync(resolve(process.cwd(), "src-tauri", "build.rs"), "utf8");
    const verify = readFileSync(resolve(process.cwd(), "scripts", "verify-android-apk.sh"), "utf8");
    const keystoreRs = readFileSync(
      resolve(process.cwd(), "crates", "pebble-crypto", "src", "android_keystore.rs"),
      "utf8",
    );
    const jni = readFileSync(resolve(process.cwd(), "src-tauri", "src", "android_jni.rs"), "utf8");

    expect(cargoConfig).toContain("max-page-size=16384");
    expect(cargoConfig).toContain("common-page-size=16384");
    expect(buildRs).toContain("max-page-size=16384");
    expect(verify).toContain("16384");
    expect(verify).toContain("llvm-readelf");
    expect(verify).toContain("app-universal-release.apk");
    expect(keystoreRs).toContain("getClassLoader");
    expect(keystoreRs).toContain("loadClass");
    expect(keystoreRs).not.toContain("find_class");
    expect(jni).toContain("getClassLoader");
    expect(jni).toContain("loadClass");
  });
});
