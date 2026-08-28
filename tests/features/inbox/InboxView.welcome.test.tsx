import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const ui = vi.hoisted(() => ({
  setActiveView: vi.fn(),
  openSettingsSection: vi.fn(),
  closeSettingsSection: vi.fn(),
}));

vi.mock("react-i18next", () => ({
  initReactI18next: { type: "3rdParty", init: vi.fn() },
  useTranslation: () => ({
    t: (_key: string, fallback?: string) => fallback ?? _key,
  }),
}));

vi.mock("@tanstack/react-query", () => ({
  useQueryClient: () => ({ invalidateQueries: vi.fn() }),
}));

vi.mock("../../../src/stores/ui.store", () => ({
  useUIStore: (selector: (state: typeof ui) => unknown) => selector(ui),
}));

vi.mock("../../../src/stores/mail.store", () => ({
  useMailStore: (selector: (state: {
    activeAccountId: null;
    activeFolderId: null;
    selectedMessageId: null;
    selectedThreadId: null;
    threadView: boolean;
    setSelectedMessage: () => void;
    setSelectedThreadId: () => void;
    toggleThreadView: () => void;
  }) => unknown) =>
    selector({
      activeAccountId: null,
      activeFolderId: null,
      selectedMessageId: null,
      selectedThreadId: null,
      threadView: false,
      setSelectedMessage: vi.fn(),
      setSelectedThreadId: vi.fn(),
      toggleThreadView: vi.fn(),
    }),
}));

vi.mock("../../../src/stores/toast.store", () => ({
  useToastStore: (selector: (state: { addToast: () => void }) => unknown) =>
    selector({ addToast: vi.fn() }),
}));

vi.mock("../../../src/hooks/queries", () => ({
  useAccountsQuery: () => ({ data: [] }),
  useFoldersForAccountsQuery: () => ({ data: [] }),
  useMessagesQuery: () => ({
    data: [],
    isLoading: false,
    hasNextPage: false,
    isFetchingNextPage: false,
    fetchNextPage: vi.fn(),
  }),
  useThreadsQuery: () => ({ data: [], isLoading: false }),
  patchMessagesCache: vi.fn(),
}));

vi.mock("../../../src/lib/api", () => ({ emptyTrash: vi.fn() }));

import InboxView from "../../../src/features/inbox/InboxView";

describe("InboxView zero-account welcome", () => {
  beforeEach(() => {
    ui.setActiveView.mockClear();
    ui.openSettingsSection.mockClear();
    ui.closeSettingsSection.mockClear();
  });

  it("offers add account, cloud restore, and the settings list", () => {
    render(<InboxView />);

    fireEvent.click(screen.getByRole("button", { name: "Add Account" }));
    expect(ui.openSettingsSection).toHaveBeenCalledWith("accounts");
    expect(ui.setActiveView).toHaveBeenCalledWith("settings");

    ui.openSettingsSection.mockClear();
    ui.setActiveView.mockClear();
    fireEvent.click(screen.getByRole("button", { name: "Restore from cloud" }));
    expect(ui.openSettingsSection).toHaveBeenCalledWith("cloudSync");
    expect(ui.setActiveView).toHaveBeenCalledWith("settings");

    ui.openSettingsSection.mockClear();
    ui.setActiveView.mockClear();
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    expect(ui.closeSettingsSection).toHaveBeenCalled();
    expect(ui.openSettingsSection).not.toHaveBeenCalled();
    expect(ui.setActiveView).toHaveBeenCalledWith("settings");
  });
});
