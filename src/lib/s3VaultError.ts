import { extractErrorMessage } from "@/lib/extractErrorMessage";

/** AEAD open failed: the vault blob is intact, but the PBKDF2 key is wrong. */
export function isS3DecryptionError(message: string): boolean {
  return /decryption failed|aead::error/i.test(message);
}

export function formatS3VaultError(
  err: unknown,
  t: (key: string, fallback?: string | Record<string, unknown>) => string,
  wrappedKey: string,
): string {
  const raw = extractErrorMessage(err);
  if (isS3DecryptionError(raw)) {
    return t(
      "cloudSync.s3PassphraseMismatch",
      "同步口令不对。请填电脑上加密云端文件时用的同一句，先保存再恢复。",
    );
  }
  return t(wrappedKey, { error: raw });
}
