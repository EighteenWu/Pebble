import { isAndroidRuntime } from "@/lib/platform";
import { logStartupTiming } from "@/lib/startupTiming";

export const DESKTOP_SPLASH_MIN_DISPLAY_MS = 2200;
export const ANDROID_SPLASH_MIN_DISPLAY_MS = 0;
export const ANDROID_SPLASH_MAX_WAIT_MS = 800;
export const SPLASH_FAILSAFE_MS = 4000;
export const DESKTOP_SPLASH_FADE_MS = 500;
export const ANDROID_SPLASH_FADE_MS = 200;

type SplashWindow = Window & {
  __splashStart?: number;
  __splashDismissed?: boolean;
};

export function splashMinDisplayMs(isAndroid: boolean): number {
  return isAndroid ? ANDROID_SPLASH_MIN_DISPLAY_MS : DESKTOP_SPLASH_MIN_DISPLAY_MS;
}

export function splashDismissDelayMs(elapsedMs: number, isAndroid: boolean): number {
  const remaining = Math.max(0, splashMinDisplayMs(isAndroid) - elapsedMs);
  return isAndroid ? Math.min(remaining, ANDROID_SPLASH_MAX_WAIT_MS) : remaining;
}

export function removeSplash(doc: Document = document): boolean {
  const splash = doc.getElementById("splash");
  doc.getElementById("still-starting")?.remove();
  doc.getElementById("splash-style")?.remove();
  if (!splash) return false;
  splash.remove();
  return true;
}

export function scheduleAppSplashDismiss(now = Date.now()): number {
  const isAndroid = isAndroidRuntime();
  const splashStart = (window as SplashWindow).__splashStart || now;
  const delay = splashDismissDelayMs(now - splashStart, isAndroid);
  const fadeMs = isAndroid ? ANDROID_SPLASH_FADE_MS : DESKTOP_SPLASH_FADE_MS;

  window.setTimeout(() => {
    logStartupTiming("splash fade started");
    const splash = document.getElementById("splash");
    if (splash) {
      splash.classList.add("fade-out");
      window.setTimeout(() => {
        removeSplash();
        (window as SplashWindow).__splashDismissed = true;
        logStartupTiming("splash removed");
      }, fadeMs);
    } else {
      document.getElementById("still-starting")?.remove();
      (window as SplashWindow).__splashDismissed = true;
      logStartupTiming("splash removed");
    }
  }, delay);

  return delay;
}
