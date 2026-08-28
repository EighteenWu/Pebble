import { describe, expect, it } from "vitest";
import {
  extractErrorMessage,
  formatDeferredServiceError,
} from "../../src/lib/extractErrorMessage";

describe("extractErrorMessage", () => {
  it("maps deferred crypto startup errors to a toast-ready message", () => {
    expect(
      formatDeferredServiceError("Device encryption is still starting. Please try again in a moment."),
    ).toMatch(/encryption is still starting|加密服务还在启动/);
    expect(
      extractErrorMessage({
        message: "Failed to initialize the device encryption key: keystore",
      }),
    ).toMatch(/Could not unlock device encryption|无法解锁设备加密/);
  });

  it("maps deferred search startup errors", () => {
    expect(formatDeferredServiceError("Search is still starting. Please try again in a moment.")).toMatch(
      /Search is still starting|搜索还在启动/,
    );
  });

  it("leaves unrelated errors unchanged", () => {
    expect(extractErrorMessage(new Error("Network error: timeout"))).toBe("Network error: timeout");
  });
});
