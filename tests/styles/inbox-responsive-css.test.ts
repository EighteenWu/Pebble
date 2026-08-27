import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

describe("inbox responsive CSS", () => {
  const css = readFileSync(join(process.cwd(), "src", "styles", "index.css"), "utf8");

  it("stacks mail list and detail panes on narrow screens", () => {
    expect(css).toMatch(/@media \(max-width:\s*760px\)/);
    expect(css).toContain('.mail-split-shell[data-has-selection="true"] .mail-list-pane');
    expect(css).toContain('.mail-split-shell[data-has-selection="false"] .mail-detail-pane');
    expect(css).toMatch(/display:\s*none\s*!important/);
  });

  it("pads the Android shell for safe areas and a slide-over sidebar", () => {
    expect(css).toContain("env(safe-area-inset-top");
    expect(css).toContain("env(safe-area-inset-bottom");
    expect(css).toContain('.app-shell[data-platform="android"] .sidebar-pane');
    expect(css).toContain("min(320px, 80vw)");
    expect(css).toContain("100dvh");
  });

  it("stacks mail panes and settings forms on Android regardless of CSS width", () => {
    expect(css).toContain('.app-shell[data-platform="android"] .mail-split-shell[data-has-selection="true"] .mail-list-pane');
    expect(css).toContain(".settings-mobile-list");
    expect(css).toContain(".s3-actions");
    expect(css).toContain('.app-shell[data-platform="android"] .s3-sync-section');
  });
});
