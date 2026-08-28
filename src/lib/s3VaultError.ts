import { extractErrorMessage } from "@/lib/extractErrorMessage";

/** AEAD open failed: the vault blob is intact, but the PBKDF2 key is wrong. */
export function isS3DecryptionError(message: string): boolean {
  return /decryption failed|aead::error/i.test(message);
}

export function formatS3VaultError(
  err: unknown,
  messages: { mismatch: string; wrap: (raw: string) => string },
): string {
  const raw = extractErrorMessage(err);
  if (isS3DecryptionError(raw)) return messages.mismatch;
  return messages.wrap(raw);
}
