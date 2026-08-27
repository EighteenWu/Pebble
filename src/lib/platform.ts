/** True when the webview is running inside the Android Tauri shell. */
export function isAndroidRuntime(userAgent = typeof navigator === "undefined" ? "" : navigator.userAgent): boolean {
  return /Android/i.test(userAgent);
}

/** Desktop window chrome, tray, and keyboard-first tools. */
export function isDesktopShell(userAgent = typeof navigator === "undefined" ? "" : navigator.userAgent): boolean {
  return !isAndroidRuntime(userAgent);
}

export function platformAttr(userAgent = typeof navigator === "undefined" ? "" : navigator.userAgent): "android" | "desktop" {
  return isAndroidRuntime(userAgent) ? "android" : "desktop";
}
