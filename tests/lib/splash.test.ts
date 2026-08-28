import { afterEach, describe, expect, it, vi } from "vitest";
import {
  ANDROID_SPLASH_FADE_MS,
  ANDROID_SPLASH_MAX_WAIT_MS,
  DESKTOP_SPLASH_MIN_DISPLAY_MS,
  SPLASH_FAILSAFE_MS,
  removeSplash,
  scheduleAppSplashDismiss,
  splashDismissDelayMs,
} from "../../src/lib/splash";

describe("splash dismiss timing", () => {
  afterEach(() => {
    delete (window as unknown as { __splashStart?: number }).__splashStart;
    vi.restoreAllMocks();
    document.body.innerHTML = "";
  });

  it("removes the Android splash on the first React frame", () => {
    expect(splashDismissDelayMs(0, true)).toBe(0);
    expect(splashDismissDelayMs(50, true)).toBe(0);
    expect(splashDismissDelayMs(0, true)).toBeLessThanOrEqual(ANDROID_SPLASH_MAX_WAIT_MS);
  });

  it("keeps the desktop splash animation window", () => {
    expect(splashDismissDelayMs(0, false)).toBe(DESKTOP_SPLASH_MIN_DISPLAY_MS);
    expect(splashDismissDelayMs(400, false)).toBe(DESKTOP_SPLASH_MIN_DISPLAY_MS - 400);
    expect(splashDismissDelayMs(3000, false)).toBe(0);
  });

  it("removes splash and still-starting fallback from the document", () => {
    document.body.innerHTML = `
      <style id="splash-style"></style>
      <div id="splash">PEBBLE</div>
      <div id="still-starting">Starting…</div>
    `;

    expect(removeSplash()).toBe(true);
    expect(document.getElementById("splash")).toBeNull();
    expect(document.getElementById("splash-style")).toBeNull();
    expect(document.getElementById("still-starting")).toBeNull();
  });

  it("schedules an immediate Android dismiss when App mounts", () => {
    vi.spyOn(navigator, "userAgent", "get").mockReturnValue("Mozilla/5.0 (Linux; Android 15)");
    document.body.innerHTML = `<div id="splash"><div class="loading-bar"></div></div>`;
    (window as unknown as { __splashStart: number }).__splashStart = Date.now();

    const delay = scheduleAppSplashDismiss();
    expect(delay).toBe(0);
    expect(ANDROID_SPLASH_FADE_MS).toBeLessThanOrEqual(800);
  });
});

describe("HTML splash failsafe", () => {
  it("force-removes #splash after 4s if React never mounts", async () => {
    const { readFileSync } = await import("node:fs");
    const { resolve } = await import("node:path");
    const html = readFileSync(resolve(process.cwd(), "index.html"), "utf8");
    expect(html).toContain("4000");
    expect(html).toContain("still-starting");
    expect(html).toContain("__splashDismissed");
    expect(SPLASH_FAILSAFE_MS).toBe(4000);
  });
});
