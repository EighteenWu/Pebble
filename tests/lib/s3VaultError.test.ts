import { describe, expect, it } from "vitest";
import { formatS3VaultError, isS3DecryptionError } from "../../src/lib/s3VaultError";

describe("s3VaultError", () => {
  it("recognizes AEAD open failures from pebble-crypto", () => {
    expect(isS3DecryptionError("Decryption failed: aead::Error")).toBe(true);
    expect(isS3DecryptionError("from cloud: Decryption failed: aead::Error")).toBe(true);
    expect(isS3DecryptionError("network timeout")).toBe(false);
  });

  it("returns the passphrase hint instead of the raw crypto error", () => {
    const message = formatS3VaultError(
      new Error("Decryption failed: aead::Error"),
      (key, fallback) => (typeof fallback === "string" ? fallback : key),
      "cloudSync.s3RestoreFailed",
    );
    expect(message).toBe("同步口令不对。请填电脑上加密云端文件时用的同一句，先保存再恢复。");
    expect(message).not.toContain("aead");
  });
});
