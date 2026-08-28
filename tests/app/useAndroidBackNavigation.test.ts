import { beforeEach, describe, expect, it } from "vitest";
import { consumeAndroidBack } from "../../src/app/useAndroidBackNavigation";
import { useComposeStore } from "../../src/stores/compose.store";
import { useMailStore } from "../../src/stores/mail.store";
import { useUIStore } from "../../src/stores/ui.store";

describe("consumeAndroidBack", () => {
  beforeEach(() => {
    useUIStore.setState({
      activeView: "inbox",
      mobileNavOpen: false,
      settingsSectionOpen: false,
    });
    useMailStore.setState({
      selectedMessageId: null,
      selectedThreadId: null,
    });
    useComposeStore.setState({ isOpen: false });
  });

  it("closes the mail drawer first", () => {
    useUIStore.setState({ mobileNavOpen: true, activeView: "settings" });
    expect(consumeAndroidBack()).toBe(true);
    expect(useUIStore.getState().mobileNavOpen).toBe(false);
    expect(useUIStore.getState().activeView).toBe("settings");
  });

  it("closes an open settings section before leaving settings", () => {
    useUIStore.setState({ activeView: "settings", settingsSectionOpen: true });
    expect(consumeAndroidBack()).toBe(true);
    expect(useUIStore.getState().settingsSectionOpen).toBe(false);
    expect(useUIStore.getState().activeView).toBe("settings");
  });

  it("leaves the settings list for inbox", () => {
    useUIStore.setState({ activeView: "settings", settingsSectionOpen: false });
    expect(consumeAndroidBack()).toBe(true);
    expect(useUIStore.getState().activeView).toBe("inbox");
  });
});
