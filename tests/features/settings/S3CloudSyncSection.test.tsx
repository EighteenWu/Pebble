import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import S3CloudSyncSection from "../../../src/features/settings/S3CloudSyncSection";
import {
  getS3SyncStatus,
  loadS3SyncConfig,
  saveS3SyncConfig,
  testS3Connection,
  syncS3Vault,
  resolveS3VaultConflict,
} from "../../../src/lib/api";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, fallback?: string | Record<string, unknown>, options?: Record<string, unknown>) => {
      const labels: Record<string, string> = {
        "cloudSync.s3Title": "S3-compatible cloud sync",
        "cloudSync.s3Provider": "Provider",
        "cloudSync.s3ProviderR2": "Cloudflare R2",
        "cloudSync.s3ProviderTos": "Volcengine TOS",
        "cloudSync.s3ProviderCustom": "Generic S3",
        "cloudSync.s3Endpoint": "Endpoint",
        "cloudSync.s3Region": "Region",
        "cloudSync.s3Bucket": "Bucket",
        "cloudSync.s3AccessKey": "Access key",
        "cloudSync.s3SecretKey": "Secret key",
        "cloudSync.s3Prefix": "Object prefix",
        "cloudSync.s3Passphrase": "Sync passphrase",
        "cloudSync.s3SaveConfig": "Save S3 sync settings",
        "cloudSync.s3ConfigSaved": "S3 sync settings saved",
        "cloudSync.s3TestConnection": "Test connection",
        "cloudSync.s3ManualSync": "Sync now",
        "cloudSync.s3Restore": "Restore from cloud",
        "cloudSync.s3UseCloud": "Use cloud",
        "cloudSync.s3UseLocal": "Use local",
        "cloudSync.s3ConflictTitle": "Cloud vault conflict",
        "cloudSync.connectionSuccess": "Connection successful",
        "common.saving": "Saving…",
      };
      if (typeof fallback === "object") {
        let value = labels[key] ?? key;
        for (const [name, replacement] of Object.entries({ ...fallback, ...options })) {
          value = value.replaceAll(`{{${name}}}`, String(replacement));
        }
        return value;
      }
      return labels[key] ?? (typeof fallback === "string" ? fallback : key);
    },
  }),
}));

vi.mock("@tanstack/react-query", () => ({
  useQueryClient: () => ({ invalidateQueries: vi.fn() }),
}));

vi.mock("../../../src/lib/api", () => ({
  loadS3SyncConfig: vi.fn(),
  getS3SyncStatus: vi.fn(),
  saveS3SyncConfig: vi.fn(),
  testS3Connection: vi.fn(),
  syncS3Vault: vi.fn(),
  restoreS3Vault: vi.fn(),
  resolveS3VaultConflict: vi.fn(),
}));

describe("S3CloudSyncSection", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(loadS3SyncConfig).mockResolvedValue(null);
    vi.mocked(saveS3SyncConfig).mockResolvedValue(undefined);
    vi.mocked(testS3Connection).mockResolvedValue("Connection successful");
    vi.mocked(getS3SyncStatus).mockResolvedValue({
      last_sync_at: null,
      revision: null,
      dirty: false,
      pending_conflict: null,
    });
  });

  it("saves R2 credentials and passphrase into secure settings", async () => {
    render(<S3CloudSyncSection />);

    fireEvent.change(await screen.findByLabelText("Endpoint"), {
      target: { value: "https://abc123.r2.cloudflarestorage.com" },
    });
    fireEvent.change(screen.getByLabelText("Bucket"), {
      target: { value: "pebble-vault" },
    });
    fireEvent.change(screen.getByLabelText("Access key"), {
      target: { value: "AKIA" },
    });
    fireEvent.change(screen.getByLabelText("Secret key"), {
      target: { value: "secret" },
    });
    fireEvent.change(screen.getByLabelText("Sync passphrase"), {
      target: { value: "correct horse" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save S3 sync settings" }));

    await waitFor(() => {
      expect(saveS3SyncConfig).toHaveBeenCalledWith(
        expect.objectContaining({
          provider: "r2",
          endpoint: "https://abc123.r2.cloudflarestorage.com",
          region: "auto",
          bucket: "pebble-vault",
          access_key: "AKIA",
          secret_key: "secret",
          prefix: "pebble",
          passphrase: "correct horse",
        }),
      );
    });
    expect(screen.getByRole("status").textContent).toContain("S3 sync settings saved");
  });

  it("fills the TOS endpoint from the region", async () => {
    render(<S3CloudSyncSection />);
    fireEvent.change(await screen.findByLabelText("Provider"), {
      target: { value: "tos" },
    });
    fireEvent.change(screen.getByLabelText("Region"), {
      target: { value: "cn-beijing" },
    });
    expect((screen.getByLabelText("Endpoint") as HTMLInputElement).value).toBe(
      "https://tos-s3-cn-beijing.volces.com",
    );
  });

  it("surfaces a cloud vs local choice instead of overwriting", async () => {
    vi.mocked(getS3SyncStatus).mockResolvedValue({
      last_sync_at: 1,
      revision: 1,
      dirty: true,
      pending_conflict: {
        local: { revision: 1, checksum: "a", device_id: "desktop-a", updated_at: 1 },
        cloud: { revision: 2, checksum: "b", device_id: "desktop-b", updated_at: 2 },
      },
    });
    vi.mocked(resolveS3VaultConflict).mockResolvedValue({
      status: "pulled",
      last_sync_at: 2,
      revision: 2,
      message: "restored",
    });

    render(<S3CloudSyncSection />);

    expect(await screen.findByRole("alertdialog")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Use cloud" }));
    await waitFor(() => {
      expect(resolveS3VaultConflict).toHaveBeenCalledWith("cloud");
    });
  });

  it("tests the connection before syncing", async () => {
    render(<S3CloudSyncSection />);
    fireEvent.change(await screen.findByLabelText("Endpoint"), {
      target: { value: "https://abc123.r2.cloudflarestorage.com" },
    });
    fireEvent.change(screen.getByLabelText("Bucket"), { target: { value: "vault" } });
    fireEvent.change(screen.getByLabelText("Access key"), { target: { value: "ak" } });
    fireEvent.change(screen.getByLabelText("Secret key"), { target: { value: "sk" } });
    fireEvent.click(screen.getByRole("button", { name: "Test connection" }));
    await waitFor(() => {
      expect(testS3Connection).toHaveBeenCalled();
    });
    expect(syncS3Vault).not.toHaveBeenCalled();
  });
});
