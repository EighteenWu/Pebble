import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { ArrowLeft, ChevronRight } from "lucide-react";
import { isAndroidRuntime } from "@/lib/platform";
import { useUIStore, type SettingsTab } from "@/stores/ui.store";
import AccountsTab from "./AccountsTab";
import GeneralTab from "./GeneralTab";
import ProxyTab from "./ProxyTab";
import AppearanceTab from "./AppearanceTab";
import CloudSyncTab from "./CloudSyncTab";
import RulesTab from "./RulesTab";
import PendingOpsTab from "./PendingOpsTab";
import ShortcutsTab from "./ShortcutsTab";
import TranslateTab from "./TranslateTab";
import PrivacyTab from "./PrivacyTab";
import AboutTab from "./AboutTab";

const TAB_IDS = ["accounts", "general", "proxy", "appearance", "privacy", "rules", "remoteWrites", "translation", "shortcuts", "cloudSync", "about"] as const;
const ANDROID_HIDDEN_TABS = new Set<SettingsTab>(["shortcuts"]);

const TAB_LABEL_KEYS: Record<string, string> = {
  accounts: "settings.accounts",
  general: "settings.general",
  proxy: "settings.proxy",
  appearance: "settings.appearance",
  privacy: "settings.privacy",
  rules: "settings.rules",
  remoteWrites: "settings.remoteWrites",
  translation: "settings.translation",
  shortcuts: "settings.shortcuts",
  cloudSync: "settings.cloudSync",
  about: "settings.about",
};

function visibleTabIds(android: boolean): readonly SettingsTab[] {
  return android ? TAB_IDS.filter((id) => !ANDROID_HIDDEN_TABS.has(id)) : TAB_IDS;
}

function SettingsTabBody({ activeTab }: { activeTab: SettingsTab }) {
  return (
    <>
      {activeTab === "accounts" && <AccountsTab />}
      {activeTab === "general" && <GeneralTab />}
      {activeTab === "proxy" && <ProxyTab />}
      {activeTab === "appearance" && <AppearanceTab />}
      {activeTab === "rules" && <RulesTab />}
      {activeTab === "remoteWrites" && <PendingOpsTab />}
      {activeTab === "translation" && <TranslateTab />}
      {activeTab === "shortcuts" && <ShortcutsTab />}
      {activeTab === "privacy" && <PrivacyTab />}
      {activeTab === "cloudSync" && <CloudSyncTab />}
      {activeTab === "about" && <AboutTab />}
    </>
  );
}

export default function SettingsView() {
  const { t } = useTranslation();
  const android = isAndroidRuntime();
  const activeTab = useUIStore((s) => s.settingsTab);
  const setSettingsTab = useUIStore((s) => s.setSettingsTab);
  const openSettingsSection = useUIStore((s) => s.openSettingsSection);
  const closeSettingsSection = useUIStore((s) => s.closeSettingsSection);
  const sectionOpen = useUIStore((s) => s.settingsSectionOpen);
  const tabs = visibleTabIds(android);
  const safeTab = tabs.includes(activeTab) ? activeTab : tabs[0];

  useEffect(() => {
    if (android && ANDROID_HIDDEN_TABS.has(activeTab)) {
      setSettingsTab("accounts");
      closeSettingsSection();
    }
  }, [android, activeTab, closeSettingsSection, setSettingsTab]);

  function handleDesktopTabChange(id: SettingsTab) {
    setSettingsTab(id);
  }

  if (android && !sectionOpen) {
    return (
      <div className="settings-mobile-list">
        <h1 className="settings-mobile-list-title">
          {t("settings.title", "Settings")}
        </h1>
        <nav aria-label={t("settings.tabs", "Settings tabs")} className="settings-mobile-nav">
          {tabs.map((id) => (
            <button
              key={id}
              type="button"
              className="settings-mobile-row"
              onClick={() => openSettingsSection(id)}
            >
              <span>{t(TAB_LABEL_KEYS[id])}</span>
              <ChevronRight size={18} aria-hidden="true" />
            </button>
          ))}
        </nav>
      </div>
    );
  }

  if (android) {
    return (
      <div className="settings-mobile-section">
        <header className="settings-mobile-section-header">
          <button
            type="button"
            className="settings-mobile-back"
            aria-label={t("common.back", "Back")}
            onClick={() => closeSettingsSection()}
          >
            <ArrowLeft size={20} />
          </button>
          <h1 className="settings-mobile-section-title">{t(TAB_LABEL_KEYS[safeTab])}</h1>
        </header>
        <div
          id={`settings-tabpanel-${safeTab}`}
          className="scroll-region settings-panel-scroll settings-mobile-panel"
          style={{
            flex: 1,
            minWidth: 0,
            padding: "16px",
            maxWidth: "none",
            boxSizing: "border-box",
            overflowY: "auto",
            overflowX: "hidden",
          }}
        >
          <SettingsTabBody activeTab={safeTab} />
        </div>
      </div>
    );
  }

  return (
    <div style={{ display: "flex", height: "100%" }}>
      <div
        role="tablist"
        aria-orientation="vertical"
        aria-label={t("settings.tabs", "Settings tabs")}
        style={{
          width: "180px",
          borderRight: "1px solid var(--color-border)",
          padding: "16px 0",
          flexShrink: 0,
        }}
      >
        {TAB_IDS.map((id, index) => (
          <button
            key={id}
            id={`settings-tab-${id}`}
            role="tab"
            aria-selected={activeTab === id}
            aria-controls={`settings-tabpanel-${id}`}
            tabIndex={activeTab === id ? 0 : -1}
            onClick={() => handleDesktopTabChange(id)}
            onKeyDown={(e) => {
              let nextIndex = index;
              if (e.key === "ArrowDown") { nextIndex = (index + 1) % TAB_IDS.length; }
              else if (e.key === "ArrowUp") { nextIndex = (index - 1 + TAB_IDS.length) % TAB_IDS.length; }
              else if (e.key === "Home") { nextIndex = 0; }
              else if (e.key === "End") { nextIndex = TAB_IDS.length - 1; }
              else { return; }
              e.preventDefault();
              handleDesktopTabChange(TAB_IDS[nextIndex]);
              document.getElementById(`settings-tab-${TAB_IDS[nextIndex]}`)?.focus();
            }}
            style={{
              display: "block",
              width: "100%",
              textAlign: "left",
              padding: "8px 20px",
              border: "none",
              background: activeTab === id ? "var(--color-bg-hover)" : "none",
              color: activeTab === id ? "var(--color-text-primary)" : "var(--color-text-secondary)",
              fontWeight: activeTab === id ? 600 : 400,
              fontSize: "13px",
              cursor: "pointer",
              borderRight: activeTab === id ? "2px solid var(--color-accent)" : "2px solid transparent",
              transition: "background-color 0.15s ease, color 0.15s ease, border-color 0.15s ease",
            }}
          >
            {t(TAB_LABEL_KEYS[id])}
          </button>
        ))}
      </div>
      <div
        id={`settings-tabpanel-${activeTab}`}
        className="scroll-region settings-panel-scroll"
        role="tabpanel"
        aria-labelledby={`settings-tab-${activeTab}`}
        style={{
          flex: 1,
          minWidth: 0,
          padding: "32px",
          maxWidth: activeTab === "remoteWrites" ? "980px" : "640px",
          boxSizing: "border-box",
          overflowY: "auto",
          overflowX: "hidden",
        }}
      >
        <SettingsTabBody activeTab={activeTab} />
      </div>
    </div>
  );
}
