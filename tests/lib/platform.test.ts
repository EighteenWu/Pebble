import { describe, expect, it } from "vitest";
import { isAndroidRuntime, isDesktopShell, platformAttr } from "../../src/lib/platform";

describe("platform helpers", () => {
  it("detects Android webviews and desktop shells from the user agent", () => {
    expect(isAndroidRuntime("Mozilla/5.0 (Linux; Android 14; Pixel) AppleWebKit/537.36")).toBe(true);
    expect(isDesktopShell("Mozilla/5.0 (Linux; Android 14; Pixel) AppleWebKit/537.36")).toBe(false);
    expect(platformAttr("Mozilla/5.0 (Linux; Android 14; Pixel) AppleWebKit/537.36")).toBe("android");

    expect(isAndroidRuntime("Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0)")).toBe(false);
    expect(isDesktopShell("Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0)")).toBe(true);
    expect(platformAttr("Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0)")).toBe("desktop");
  });
});
