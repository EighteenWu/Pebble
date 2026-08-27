import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  inputStyle as baseInputStyle,
  labelStyle as baseLabelStyle,
  fieldGroupStyle,
} from "../../styles/form";
import { useQueryClient } from "@tanstack/react-query";
import {
  getS3SyncStatus,
  loadS3SyncConfig,
  resolveS3VaultConflict,
  restoreS3Vault,
  saveS3SyncConfig,
  syncS3Vault,
  testS3Connection,
  type S3Provider,
  type S3SyncConfig,
  type VaultConflict,
  type VaultSyncResult,
} from "../../lib/api";
import { extractErrorMessage as errorMessage } from "@/lib/extractErrorMessage";
import { formatS3VaultError } from "@/lib/s3VaultError";
import { useToastStore } from "@/stores/toast.store";

const labelStyle: React.CSSProperties = {
  ...baseLabelStyle,
  fontWeight: 500,
};

const inputStyle: React.CSSProperties = {
  ...baseInputStyle,
  padding: "8px 10px",
  backgroundColor: "var(--color-bg-secondary)",
};

const buttonStyle: React.CSSProperties = {
  padding: "8px 18px",
  fontSize: "13px",
  fontWeight: 500,
  border: "none",
  borderRadius: "6px",
  cursor: "pointer",
};

function tosEndpoint(region: string): string {
  const trimmed = region.trim();
  return trimmed ? `https://tos-s3-${trimmed}.volces.com` : "";
}

function emptyConfig(): S3SyncConfig {
  return {
    provider: "r2",
    endpoint: "",
    region: "auto",
    bucket: "",
    access_key: "",
    secret_key: "",
    prefix: "pebble",
    passphrase: "",
    enabled: false,
    interval_minutes: 60,
  };
}

function applyProviderDefaults(config: S3SyncConfig, provider: S3Provider): S3SyncConfig {
  const next = { ...config, provider };
  if (provider === "r2") {
    next.region = next.region.trim() && next.region !== "" ? next.region : "auto";
    if (!next.region || next.region === "") next.region = "auto";
    if (next.region !== "auto" && !next.endpoint.includes("r2.cloudflarestorage.com")) {
      next.region = "auto";
    }
    if (!next.endpoint) {
      next.region = "auto";
    }
  }
  if (provider === "tos") {
    if (next.region === "auto") next.region = "";
    if (!next.endpoint || next.endpoint.includes("r2.cloudflarestorage.com")) {
      next.endpoint = tosEndpoint(next.region);
    }
  }
  return next;
}

function formatSyncTime(epoch: number | null | undefined): string | null {
  if (!epoch) return null;
  return new Date(epoch * 1000).toLocaleString();
}

function describeResult(
  result: VaultSyncResult,
  messages: { synced: string; pulled: string; empty: string; conflict: string },
): { type: "success" | "error"; message: string; conflict?: VaultConflict } {
  switch (result.status) {
    case "synced":
      return { type: "success", message: messages.synced };
    case "pulled":
      return { type: "success", message: messages.pulled };
    case "empty":
      return { type: "success", message: messages.empty };
    case "conflict":
      return {
        type: "error",
        message: messages.conflict,
        conflict: { local: result.local, cloud: result.cloud },
      };
  }
}

export default function S3CloudSyncSection() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const addToast = useToastStore((s) => s.addToast);
  const [config, setConfig] = useState<S3SyncConfig>(emptyConfig);
  const [loaded, setLoaded] = useState(false);
  const [lastSyncAt, setLastSyncAt] = useState<number | null>(null);
  const [conflict, setConflict] = useState<VaultConflict | null>(null);
  const [statusMsg, setStatusMsg] = useState("");
  const [statusType, setStatusType] = useState<"success" | "error" | "">("");
  const [testing, setTesting] = useState(false);
  const [saving, setSaving] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [restoring, setRestoring] = useState(false);

  useEffect(() => {
    Promise.all([loadS3SyncConfig(), getS3SyncStatus()])
      .then(([saved, status]) => {
        if (saved) setConfig(saved);
        setLastSyncAt(status.last_sync_at);
        setConflict(status.pending_conflict);
      })
      .catch(() => {})
      .finally(() => setLoaded(true));
  }, []);

  const busy = testing || saving || syncing || restoring;
  const credentialsReady = Boolean(
    (config.provider !== "r2" || config.endpoint.trim()) &&
      (config.provider !== "custom" || config.endpoint.trim()) &&
      (config.provider !== "tos" || config.region.trim() || config.endpoint.trim()) &&
      config.bucket.trim() &&
      config.access_key.trim() &&
      config.secret_key,
  );

  function applyResult(result: VaultSyncResult) {
    const described = describeResult(result, {
      synced: t("cloudSync.s3SyncSuccess", "Settings vault uploaded to your bucket"),
      pulled: t(
        "cloudSync.s3PullSuccess",
        "Settings vault restored from cloud. Accounts can receive mail with the same passphrase.",
      ),
      empty: t("cloudSync.s3Empty", "No vault found in the bucket yet. Sync now to create one."),
      conflict: t(
        "cloudSync.s3ConflictPrompt",
        "A newer cloud vault conflicts with local changes. Choose Use cloud or Use local.",
      ),
    });
    setStatusMsg(described.message);
    setStatusType(described.type);
    if (result.status === "synced" || result.status === "pulled") {
      setLastSyncAt(result.last_sync_at);
      setConflict(null);
    }
    if (described.conflict) setConflict(described.conflict);
    if (result.status === "pulled") {
      void queryClient.invalidateQueries();
    }
  }

  async function handleSave() {
    setStatusMsg("");
    if (config.enabled && !credentialsReady) {
      setStatusMsg(
        t(
          "cloudSync.s3CredentialsRequired",
          "Enter endpoint, region, bucket, access key, and secret key before testing or enabling sync.",
        ),
      );
      setStatusType("error");
      return;
    }
    if ((config.enabled || config.passphrase) && !config.passphrase.trim()) {
      setStatusMsg(
        t(
          "cloudSync.s3PassphraseRequired",
          "Enter a sync passphrase. Cloud objects are always encrypted with this passphrase.",
        ),
      );
      setStatusType("error");
      return;
    }
    setSaving(true);
    try {
      await saveS3SyncConfig(config);
      setStatusMsg(t("cloudSync.s3ConfigSaved", "S3 sync settings saved"));
      setStatusType("success");
    } catch (err: unknown) {
      setStatusMsg(
        t("cloudSync.s3ConfigSaveFailed", { error: errorMessage(err) }),
      );
      setStatusType("error");
    } finally {
      setSaving(false);
    }
  }

  async function handleTest() {
    setStatusMsg("");
    if (!credentialsReady) {
      setStatusMsg(
        t(
          "cloudSync.s3CredentialsRequired",
          "Enter endpoint, region, bucket, access key, and secret key before testing or enabling sync.",
        ),
      );
      setStatusType("error");
      return;
    }
    setTesting(true);
    try {
      await testS3Connection(config);
      setStatusMsg(t("cloudSync.connectionSuccess"));
      setStatusType("success");
    } catch (err: unknown) {
      setStatusMsg(`${t("cloudSync.connectionFailed")}: ${errorMessage(err)}`);
      setStatusType("error");
    } finally {
      setTesting(false);
    }
  }

  async function handleSync() {
    setSyncing(true);
    setStatusMsg("");
    try {
      await saveS3SyncConfig({ ...config, enabled: config.enabled });
      applyResult(await syncS3Vault());
    } catch (err: unknown) {
      const message = formatS3VaultError(err, t, "cloudSync.s3SyncFailed");
      setStatusMsg(message);
      setStatusType("error");
    } finally {
      setSyncing(false);
    }
  }

  async function handleRestore() {
    setRestoring(true);
    setStatusMsg("");
    try {
      await saveS3SyncConfig(config);
      applyResult(await restoreS3Vault());
    } catch (err: unknown) {
      const message = formatS3VaultError(err, t, "cloudSync.s3RestoreFailed");
      setStatusMsg(message);
      setStatusType("error");
      addToast({ message, type: "error" });
    } finally {
      setRestoring(false);
    }
  }

  async function handleResolve(choice: "cloud" | "local") {
    setSyncing(true);
    setStatusMsg("");
    try {
      applyResult(await resolveS3VaultConflict(choice));
    } catch (err: unknown) {
      setStatusMsg(formatS3VaultError(err, t, "cloudSync.s3SyncFailed"));
      setStatusType("error");
    } finally {
      setSyncing(false);
    }
  }

  const endpointPlaceholder =
    config.provider === "r2"
      ? t("cloudSync.s3EndpointR2Placeholder", "https://<ACCOUNT_ID>.r2.cloudflarestorage.com")
      : config.provider === "tos"
        ? t("cloudSync.s3EndpointTosPlaceholder", "https://tos-s3-<region>.volces.com")
        : t("cloudSync.s3EndpointCustomPlaceholder", "https://s3.example.com");

  if (!loaded) return null;

  return (
    <div className="s3-sync-section" style={{ marginTop: "28px", paddingTop: "8px", borderTop: "1px solid var(--color-border)" }}>
      <h2
        style={{
          fontSize: "18px",
          fontWeight: 600,
          color: "var(--color-text-primary)",
          marginTop: 0,
          marginBottom: "16px",
        }}
      >
        {t("cloudSync.s3Title", "S3-compatible cloud sync")}
      </h2>
      <p style={{ fontSize: "13px", lineHeight: 1.5, color: "var(--color-text-secondary)", maxWidth: "640px" }}>
        {t(
          "cloudSync.s3Description",
          "Store the encrypted settings vault in your own Cloudflare R2, Volcengine TOS, or generic S3 bucket. The same sync passphrase is required to restore accounts and receive mail on another desktop. Pebble does not provide a hosted bucket.",
        )}
      </p>
      <p
        style={{
          fontSize: "13px",
          lineHeight: 1.5,
          color: "var(--color-text-secondary)",
          maxWidth: "640px",
          padding: "8px 12px",
          background: "var(--color-bg-secondary)",
          borderRadius: "6px",
          borderLeft: "3px solid var(--color-accent)",
        }}
      >
        {t(
          "cloudSync.s3ScopeNotice",
          "Only the passphrase-encrypted settings vault is uploaded (accounts metadata, rules, Kanban, contacts, and wrapped IMAP/OAuth secrets). SQLite, the search index, mail bodies, and attachments stay on this device.",
        )}
      </p>

      <div style={fieldGroupStyle}>
        <label htmlFor="s3-provider" style={labelStyle}>{t("cloudSync.s3Provider", "Provider")}</label>
        <select
          id="s3-provider"
          name="s3_provider"
          style={inputStyle}
          value={config.provider}
          disabled={saving}
          onChange={(event) => {
            const provider = event.target.value as S3Provider;
            setConfig((current) => applyProviderDefaults(current, provider));
          }}
        >
          <option value="r2">{t("cloudSync.s3ProviderR2", "Cloudflare R2")}</option>
          <option value="tos">{t("cloudSync.s3ProviderTos", "Volcengine TOS")}</option>
          <option value="custom">{t("cloudSync.s3ProviderCustom", "Generic S3")}</option>
        </select>
      </div>

      <div style={fieldGroupStyle}>
        <label htmlFor="s3-endpoint" style={labelStyle}>{t("cloudSync.s3Endpoint", "Endpoint")}</label>
        <input
          id="s3-endpoint"
          name="s3_endpoint"
          style={inputStyle}
          value={config.endpoint}
          disabled={saving}
          placeholder={endpointPlaceholder}
          autoComplete="off"
          onChange={(event) => setConfig((current) => ({ ...current, endpoint: event.target.value }))}
        />
      </div>

      <div style={fieldGroupStyle}>
        <label htmlFor="s3-region" style={labelStyle}>{t("cloudSync.s3Region", "Region")}</label>
        <input
          id="s3-region"
          name="s3_region"
          style={inputStyle}
          value={config.region}
          disabled={saving || config.provider === "r2"}
          placeholder={config.provider === "r2" ? "auto" : "cn-beijing"}
          autoComplete="off"
          onChange={(event) => {
            const region = event.target.value;
            setConfig((current) => ({
              ...current,
              region,
              endpoint:
                current.provider === "tos" &&
                (!current.endpoint || current.endpoint.startsWith("https://tos-s3-"))
                  ? tosEndpoint(region)
                  : current.endpoint,
            }));
          }}
        />
        {config.provider === "r2" && (
          <span style={{ fontSize: "12px", color: "var(--color-text-secondary)" }}>
            {t("cloudSync.s3RegionR2Hint", "R2 uses region auto")}
          </span>
        )}
      </div>

      <div style={fieldGroupStyle}>
        <label htmlFor="s3-bucket" style={labelStyle}>{t("cloudSync.s3Bucket", "Bucket")}</label>
        <input
          id="s3-bucket"
          name="s3_bucket"
          style={inputStyle}
          value={config.bucket}
          disabled={saving}
          autoComplete="off"
          onChange={(event) => setConfig((current) => ({ ...current, bucket: event.target.value }))}
        />
      </div>

      <div style={fieldGroupStyle}>
        <label htmlFor="s3-access-key" style={labelStyle}>{t("cloudSync.s3AccessKey", "Access key")}</label>
        <input
          id="s3-access-key"
          name="s3_access_key"
          style={inputStyle}
          value={config.access_key}
          disabled={saving}
          autoComplete="off"
          onChange={(event) => setConfig((current) => ({ ...current, access_key: event.target.value }))}
        />
      </div>

      <div style={fieldGroupStyle}>
        <label htmlFor="s3-secret-key" style={labelStyle}>{t("cloudSync.s3SecretKey", "Secret key")}</label>
        <input
          id="s3-secret-key"
          name="s3_secret_key"
          type="password"
          style={inputStyle}
          value={config.secret_key}
          disabled={saving}
          autoComplete="new-password"
          onChange={(event) => setConfig((current) => ({ ...current, secret_key: event.target.value }))}
        />
      </div>

      <div style={fieldGroupStyle}>
        <label htmlFor="s3-prefix" style={labelStyle}>{t("cloudSync.s3Prefix", "Object prefix")}</label>
        <input
          id="s3-prefix"
          name="s3_prefix"
          style={inputStyle}
          value={config.prefix}
          disabled={saving}
          placeholder="pebble"
          autoComplete="off"
          onChange={(event) => setConfig((current) => ({ ...current, prefix: event.target.value }))}
        />
        <span style={{ fontSize: "12px", color: "var(--color-text-secondary)" }}>
          {t("cloudSync.s3PrefixHint", "Objects are written as {prefix}/vault.json and {prefix}/vault.json.meta")}
        </span>
      </div>

      <div style={fieldGroupStyle}>
        <label htmlFor="s3-passphrase" style={labelStyle}>{t("cloudSync.s3Passphrase", "Sync passphrase")}</label>
        <input
          id="s3-passphrase"
          name="s3_passphrase"
          type="password"
          style={inputStyle}
          value={config.passphrase}
          disabled={saving}
          placeholder={t(
            "cloudSync.s3PassphrasePlaceholder",
            "Required. Encrypts the cloud vault; never the device key",
          )}
          autoComplete="new-password"
          onChange={(event) => setConfig((current) => ({ ...current, passphrase: event.target.value }))}
        />
      </div>

      <div style={{ display: "flex", alignItems: "center", gap: "10px", marginBottom: "10px" }}>
        <label style={{ display: "flex", alignItems: "center", gap: "8px", fontSize: "13px", color: "var(--color-text-primary)", cursor: "pointer" }}>
          <input
            type="checkbox"
            checked={config.enabled}
            disabled={saving || (!config.enabled && !credentialsReady)}
            onChange={(event) => setConfig((current) => ({ ...current, enabled: event.target.checked }))}
          />
          {t("cloudSync.s3Enable", "Enable automatic cloud sync")}
        </label>
      </div>
      {config.enabled && (
        <div style={{ display: "flex", alignItems: "center", gap: "8px", marginBottom: "10px" }}>
          <label htmlFor="s3-interval" style={{ fontSize: "12px", color: "var(--color-text-secondary)" }}>
            {t("cloudSync.autoBackupInterval", "Interval")}:
          </label>
          <select
            id="s3-interval"
            value={config.interval_minutes}
            disabled={saving}
            style={{ ...inputStyle, width: "auto", padding: "4px 8px" }}
            onChange={(event) =>
              setConfig((current) => ({ ...current, interval_minutes: Number(event.target.value) }))
            }
          >
            <option value={30}>30 min</option>
            <option value={60}>1 h</option>
            <option value={180}>3 h</option>
            <option value={360}>6 h</option>
            <option value={720}>12 h</option>
            <option value={1440}>24 h</option>
          </select>
        </div>
      )}
      {!credentialsReady && (
        <p style={{ fontSize: "12px", color: "var(--color-text-secondary)", margin: "0 0 12px" }}>
          {t("cloudSync.s3FillFields", "Fill in bucket credentials and a sync passphrase to enable automatic sync.")}
        </p>
      )}

      <div className="s3-actions" style={{ display: "flex", flexWrap: "wrap", gap: "10px", marginTop: "12px" }}>
        <button
          type="button"
          style={{ ...buttonStyle, background: "var(--color-accent)", color: "#fff", opacity: busy ? 0.6 : 1 }}
          disabled={busy}
          onClick={() => void handleSave()}
        >
          {saving ? t("common.saving") : t("cloudSync.s3SaveConfig", "Save S3 sync settings")}
        </button>
        <button
          type="button"
          style={{ ...buttonStyle, background: "var(--color-bg-hover)", color: "var(--color-text-primary)", opacity: busy ? 0.6 : 1 }}
          disabled={busy}
          onClick={() => void handleTest()}
        >
          {testing ? t("common.testing") : t("cloudSync.s3TestConnection", "Test connection")}
        </button>
        <button
          type="button"
          style={{ ...buttonStyle, background: "var(--color-accent)", color: "#fff", opacity: busy ? 0.6 : 1 }}
          disabled={busy}
          onClick={() => void handleSync()}
        >
          {syncing ? t("common.saving") : t("cloudSync.s3ManualSync", "Sync now")}
        </button>
        <button
          type="button"
          style={{ ...buttonStyle, background: "var(--color-bg-hover)", color: "var(--color-text-primary)", opacity: busy ? 0.6 : 1 }}
          disabled={busy}
          onClick={() => void handleRestore()}
        >
          {restoring ? t("common.loading") : t("cloudSync.s3Restore", "Restore from cloud")}
        </button>
      </div>

      <div style={{ marginTop: "14px", fontSize: "12px", color: "var(--color-text-secondary)" }}>
        {t("cloudSync.s3LastSync", "Last sync")}:{" "}
        {formatSyncTime(lastSyncAt) ?? t("cloudSync.s3NeverSynced", "Not synced yet")}
      </div>

      {conflict && (
        <div
          role="alertdialog"
          aria-labelledby="s3-conflict-title"
          style={{
            marginTop: "14px",
            padding: "12px 14px",
            borderRadius: "6px",
            fontSize: "13px",
            background: "rgba(220, 53, 69, 0.1)",
            color: "#dc3545",
            border: "1px solid rgba(220, 53, 69, 0.3)",
          }}
        >
          <div id="s3-conflict-title" style={{ fontWeight: 600, marginBottom: "8px" }}>
            {t("cloudSync.s3ConflictTitle", "Cloud vault conflict")}
          </div>
          <div style={{ whiteSpace: "pre-wrap", marginBottom: "10px" }}>
            {t("cloudSync.s3ConflictMessage", {
              localRevision: conflict.local.revision,
              localDevice: conflict.local.device_id,
              cloudRevision: conflict.cloud.revision,
              cloudDevice: conflict.cloud.device_id,
            })}
          </div>
          <div style={{ display: "flex", gap: "8px" }}>
            <button
              type="button"
              style={{ ...buttonStyle, background: "var(--color-bg-hover)", color: "var(--color-text-primary)" }}
              disabled={busy}
              onClick={() => void handleResolve("cloud")}
            >
              {t("cloudSync.s3UseCloud", "Use cloud")}
            </button>
            <button
              type="button"
              style={{ ...buttonStyle, background: "var(--color-accent)", color: "#fff" }}
              disabled={busy}
              onClick={() => void handleResolve("local")}
            >
              {t("cloudSync.s3UseLocal", "Use local")}
            </button>
          </div>
        </div>
      )}

      {statusMsg && (
        <div
          role={statusType === "error" ? "alert" : "status"}
          aria-live="polite"
          style={{
            marginTop: "14px",
            padding: "10px 14px",
            borderRadius: "6px",
            fontSize: "13px",
            background: statusType === "success" ? "var(--color-bg-hover)" : "rgba(220, 53, 69, 0.1)",
            color: statusType === "success" ? "var(--color-text-primary)" : "#dc3545",
            border: `1px solid ${statusType === "success" ? "var(--color-border)" : "rgba(220, 53, 69, 0.3)"}`,
          }}
        >
          {statusMsg}
        </div>
      )}

    </div>
  );
}
