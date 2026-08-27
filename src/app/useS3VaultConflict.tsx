import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { useQueryClient } from "@tanstack/react-query";
import ConfirmDialog from "@/components/ConfirmDialog";
import { resolveS3VaultConflict, type VaultConflict } from "@/lib/api";

export function S3VaultConflictListener() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [conflict, setConflict] = useState<VaultConflict | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    const unlisten = listen<VaultConflict>("cloud-sync:vault-conflict", (event) => {
      setConflict(event.payload);
    });
    const restored = listen("cloud-sync:vault-restored", () => {
      void queryClient.invalidateQueries();
    });
    return () => {
      unlisten.then((fn) => fn());
      restored.then((fn) => fn());
    };
  }, [queryClient]);

  if (!conflict) return null;

  return (
    <ConfirmDialog
      title={t("cloudSync.s3ConflictTitle", "Cloud vault conflict")}
      message={t("cloudSync.s3ConflictMessage", {
        localRevision: conflict.local.revision,
        localDevice: conflict.local.device_id,
        cloudRevision: conflict.cloud.revision,
        cloudDevice: conflict.cloud.device_id,
      })}
      confirmLabel={t("cloudSync.s3UseLocal", "Use local")}
      cancelLabel={t("cloudSync.s3UseCloud", "Use cloud")}
      busy={busy}
      onCancel={() => {
        setBusy(true);
        resolveS3VaultConflict("cloud")
          .then((result) => {
            if (result.status !== "conflict") setConflict(null);
          })
          .finally(() => setBusy(false));
      }}
      onConfirm={() => {
        setBusy(true);
        resolveS3VaultConflict("local")
          .then((result) => {
            if (result.status !== "conflict") setConflict(null);
          })
          .finally(() => setBusy(false));
      }}
    />
  );
}
