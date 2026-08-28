import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import SettingsView from "../../../src/features/settings/SettingsView";
import { useUIStore } from "../../../src/stores/ui.store";

const androidRuntime = vi.hoisted(() => ({ value: false }));

vi.mock("../../../src/lib/platform", () => ({
  isAndroidRuntime: () => androidRuntime.value,
}));

vi.mock("react-i18next", () => ({
  initReactI18next: {
    type: "3rdParty",
    init: vi.fn(),
  },
  useTranslation: () => ({
    t: (key: string, fallback?: string) => {
      const labels: Record<string, string> = {
        "settings.accounts": "Accounts",
        "settings.general": "General",
        "settings.proxy": "Proxy",
        "settings.appearance": "Appearance",
        "settings.privacy": "Privacy",
        "settings.rules": "Rules",
        "settings.remoteWrites": "Remote Writes",
        "settings.translation": "Translation",
        "settings.shortcuts": "Shortcuts",
        "settings.cloudSync": "Settings Backup",
        "settings.about": "About",
        "settings.tabs": "Settings tabs",
        "settings.title": "Settings",
        "common.back": "Back",
      };
      return labels[key] ?? fallback ?? key;
    },
  }),
}));

vi.mock("../../../src/features/settings/AccountsTab", () => ({
  default: () => <div>Accounts panel</div>,
}));

vi.mock("../../../src/features/settings/GeneralTab", () => ({
  default: () => <div>General panel</div>,
}));

vi.mock("../../../src/features/settings/ProxyTab", () => ({
  default: () => <div>Proxy panel</div>,
}));

vi.mock("../../../src/features/settings/AppearanceTab", () => ({
  default: () => <div>Appearance panel</div>,
}));

vi.mock("../../../src/features/settings/CloudSyncTab", () => ({
  default: () => <div>Cloud sync panel</div>,
}));

vi.mock("../../../src/features/settings/RulesTab", () => ({
  default: () => <div>Rules panel</div>,
}));

vi.mock("../../../src/features/settings/ShortcutsTab", () => ({
  default: () => <div>Shortcuts panel</div>,
}));

vi.mock("../../../src/features/settings/TranslateTab", () => ({
  default: () => <div>Translation panel</div>,
}));

vi.mock("../../../src/features/settings/PrivacyTab", () => ({
  default: () => <div>Privacy panel</div>,
}));

vi.mock("../../../src/features/settings/AboutTab", () => ({
  default: () => <div>About panel</div>,
}));

vi.mock("../../../src/features/settings/PendingOpsTab", () => ({
  default: () => <div>Remote queue panel</div>,
}));

describe("SettingsView", () => {
  beforeEach(() => {
    androidRuntime.value = false;
    useUIStore.setState({ settingsTab: "accounts", settingsSectionOpen: false });
  });

  it("exposes the pending remote writes queue as a settings tab", () => {
    render(<SettingsView />);

    const tab = screen.getByRole("tab", { name: "Remote Writes" });
    fireEvent.click(tab);

    expect(tab.getAttribute("aria-selected")).toBe("true");
    expect(screen.getByText("Remote queue panel")).toBeTruthy();
  });

  it("exposes global proxy settings as a dedicated settings tab", () => {
    render(<SettingsView />);

    const tab = screen.getByRole("tab", { name: "Proxy" });
    fireEvent.click(tab);

    expect(tab.getAttribute("aria-selected")).toBe("true");
    expect(screen.getByText("Proxy panel")).toBeTruthy();
  });

  it("does not animate all properties on settings tabs", () => {
    render(<SettingsView />);

    const tab = screen.getByRole("tab", { name: "Accounts" });

    expect(tab.style.transition).not.toContain("all");
    expect(tab.style.transition).toContain("background-color");
    expect(tab.style.transition).toContain("border-color");
  });

  it("keeps the settings panel vertically scrollable without horizontal overflow", () => {
    render(<SettingsView />);

    const panel = screen.getByRole("tabpanel");

    expect(panel.className).toContain("settings-panel-scroll");
    expect(panel.style.overflowY).toBe("auto");
    expect(panel.style.overflowX).toBe("hidden");
    expect(panel.style.boxSizing).toBe("border-box");
  });

  it("uses a full-width section list on Android instead of a persistent tab rail", () => {
    androidRuntime.value = true;
    render(<SettingsView />);

    expect(screen.queryByRole("tablist")).toBeNull();
    expect(screen.getByRole("heading", { name: "Settings" })).toBeTruthy();
    expect(screen.getByRole("button", { name: /Settings Backup/ })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Shortcuts" })).toBeNull();
    expect(screen.queryByText("Accounts panel")).toBeNull();
  });

  it("opens a settings section full width on Android and returns to the list", () => {
    androidRuntime.value = true;
    render(<SettingsView />);

    fireEvent.click(screen.getByRole("button", { name: /Settings Backup/ }));

    expect(screen.getByText("Cloud sync panel")).toBeTruthy();
    expect(screen.queryByRole("tablist")).toBeNull();
    expect(screen.queryByRole("button", { name: "Shortcuts" })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Back" }));

    expect(screen.queryByText("Cloud sync panel")).toBeNull();
    expect(screen.getByRole("heading", { name: "Settings" })).toBeTruthy();
  });
});
