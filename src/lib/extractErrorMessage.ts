import i18next from "i18next";

/** Extract a human-readable error message from an unknown catch value. */
export function extractErrorMessage(err: unknown): string {
  let raw = "Unknown error";
  if (err instanceof Error) raw = err.message;
  else if (typeof err === "string") raw = err;
  else if (err && typeof err === "object" && "message" in err) {
    raw = String((err as { message: unknown }).message);
  }
  return formatDeferredServiceError(raw);
}

function translateStartup(key: string, fallback: string): string {
  const translated = i18next.t(key, { defaultValue: fallback });
  return typeof translated === "string" && translated.length > 0 ? translated : fallback;
}

export function formatDeferredServiceError(raw: string): string {
  if (/Device encryption is still starting/i.test(raw)) {
    return translateStartup(
      "startup.cryptoStarting",
      "Device encryption is still starting. Try again in a moment.",
    );
  }
  if (/Failed to initialize the device encryption key/i.test(raw)) {
    return translateStartup(
      "startup.cryptoFailed",
      "Could not unlock device encryption. Try again, or restart the app.",
    );
  }
  if (/Search is still starting/i.test(raw)) {
    return translateStartup(
      "startup.searchStarting",
      "Search is still starting. Try again in a moment.",
    );
  }
  return raw;
}
