import { render } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mockMailState = {
  activeAccountId: "account-1",
  activeFolderId: "folder-inbox",
  selectedMessageId: "msg-1" as string | null,
  selectedThreadId: null as string | null,
  threadView: false,
  setSelectedMessage: vi.fn(),
  setSelectedThreadId: vi.fn(),
  toggleThreadView: vi.fn(),
};

vi.mock("react-i18next", () => ({
  initReactI18next: { type: "3rdParty", init: vi.fn() },
  useTranslation: () => ({
    t: (_key: string, fallback?: string) => fallback ?? _key,
  }),
}));

vi.mock("@tanstack/react-query", () => ({
  useQueryClient: () => ({ invalidateQueries: vi.fn() }),
}));

vi.mock("@tanstack/react-virtual", () => ({
  useVirtualizer: () => ({
    getTotalSize: () => 0,
    getVirtualItems: () => [],
    measureElement: vi.fn(),
    scrollToIndex: vi.fn(),
  }),
}));

vi.mock("../../../src/stores/mail.store", () => ({
  useMailStore: (selector: (state: typeof mockMailState) => unknown) => selector(mockMailState),
}));

vi.mock("../../../src/hooks/queries", () => ({
  useAccountsQuery: () => ({ data: [{ id: "account-1" }] }),
  useFoldersForAccountsQuery: () => ({
    data: [{ id: "folder-inbox", role: "inbox" }],
  }),
  useMessagesQuery: () => ({
    data: [{ id: "msg-1", subject: "Hello" }],
    isLoading: false,
    hasNextPage: false,
    isFetchingNextPage: false,
    fetchNextPage: vi.fn(),
  }),
  useThreadsQuery: () => ({ data: [], isLoading: false }),
  patchMessagesCache: vi.fn(),
}));

vi.mock("../../../src/components/SearchBar", () => ({ default: () => <div>Search bar</div> }));
vi.mock("../../../src/components/MessageList", () => ({ default: () => <div>Message list</div> }));
vi.mock("../../../src/components/MessageDetail", () => ({ default: () => <div>Message detail</div> }));
vi.mock("../../../src/features/inbox/ThreadView", () => ({ default: () => <div>Thread detail</div> }));
vi.mock("../../../src/components/ConfirmDialog", () => ({ default: () => null }));
vi.mock("../../../src/lib/api", () => ({ emptyTrash: vi.fn() }));

import InboxView from "../../../src/features/inbox/InboxView";

describe("InboxView responsive split", () => {
  beforeEach(() => {
    mockMailState.selectedMessageId = "msg-1";
    mockMailState.threadView = false;
  });

  it("marks the inbox shell selected so narrow CSS can stack list and detail", () => {
    const { container } = render(<InboxView />);
    const shell = container.querySelector(".mail-split-shell");
    expect(shell?.getAttribute("data-has-selection")).toBe("true");
    expect(container.querySelector(".mail-list-pane")).toBeTruthy();
    expect(container.querySelector(".mail-detail-pane")).toBeTruthy();
  });
});
