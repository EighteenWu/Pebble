import { useEffect } from "react";
import { onBackButtonPress } from "@tauri-apps/api/app";
import { isAndroidRuntime } from "@/lib/platform";
import { useComposeStore } from "@/stores/compose.store";
import { useMailStore } from "@/stores/mail.store";
import { useUIStore } from "@/stores/ui.store";

export function consumeAndroidBack(): boolean {
  const ui = useUIStore.getState();
  const mail = useMailStore.getState();

  if (ui.mobileNavOpen) {
    ui.closeMobileNav();
    return true;
  }

  if (ui.activeView === "settings" && ui.settingsSectionOpen) {
    ui.closeSettingsSection();
    return true;
  }

  if (ui.activeView === "settings") {
    ui.closeSettingsSection();
    ui.setActiveView("inbox");
    return true;
  }

  if (ui.activeView === "compose") {
    useComposeStore.getState().closeCompose();
    return true;
  }

  if (mail.selectedMessageId || mail.selectedThreadId) {
    mail.setSelectedMessage(null);
    mail.setSelectedThreadId(null);
    return true;
  }

  return false;
}

/** Consume Android system Back as overlay / list→detail, not app exit. */
export function useAndroidBackNavigation() {
  useEffect(() => {
    if (!isAndroidRuntime()) return;

    let disposed = false;
    let unregister: (() => void) | undefined;

    onBackButtonPress(({ canGoBack }) => {
      if (consumeAndroidBack()) return;
      if (canGoBack) {
        window.history.back();
      }
    })
      .then((listener) => {
        if (disposed) {
          void listener.unregister();
          return;
        }
        unregister = () => {
          void listener.unregister();
        };
      })
      .catch(() => {});

    return () => {
      disposed = true;
      unregister?.();
    };
  }, []);
}
