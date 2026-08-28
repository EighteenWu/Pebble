import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("frontend bootstrap", () => {
  it("does not gate first paint on profile namespace IPC", () => {
    const src = readFileSync(resolve(process.cwd(), "src/main.tsx"), "utf8");
    expect(src).toMatch(/void initializeProfileStorageNamespace\(\)/);
    expect(src).not.toMatch(/await initializeProfileStorageNamespace/);
  });

  it("loads i18n in parallel with the App module", () => {
    const src = readFileSync(resolve(process.cwd(), "src/main.tsx"), "utf8");
    expect(src).toMatch(/Promise\.all/);
    expect(src).toMatch(/import\("@\/lib\/i18n"\)/);
  });
});
