import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isAndroidRuntime } from "@/lib/platform";
import { useComposeStore } from "@/stores/compose.store";
import { useMailStore } from "@/stores/mail.store";
import { useUIStore } from "@/stores/ui.store";

/** Consume Android system Back as overlay / list→detail, not app exit. */
export function useAndroidBackNavigation() {
  useEffect(() => {
    if (!isAndroidRuntime()) return;

    let unlisten: (() => void) | undefined;
    let disposed = false;

    const appWindow = getCurrentWindow() as {
      onBackRequested?: (handler: (event: { preventDefault: () => void }) => void) => Promise<() => void>;
    };
    if (!appWindow.onBackRequested) return;

    appWindow
      .onBackRequested((event) => {
        const ui = useUIStore.getState();
        const mail = useMailStore.getState();

        if (ui.mobileNavOpen) {
          event.preventDefault();
          ui.closeMobileNav();
          return;
        }

        if (ui.activeView === "settings" && ui.settingsSectionOpen) {
          event.preventDefault();
          ui.closeSettingsSection();
          return;
        }

        if (ui.activeView === "compose") {
          event.preventDefault();
          useComposeStore.getState().closeCompose();
          return;
        }

        if (mail.selectedMessageId || mail.selectedThreadId) {
          event.preventDefault();
          mail.setSelectedMessage(null);
          mail.setSelectedThreadId(null);
          return;
        }
      })
      .then((fn) => {
        if (disposed) {
          fn();
          return;
        }
        unlisten = fn;
      })
      .catch(() => {});

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);
}
