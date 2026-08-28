import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClientProvider } from "@tanstack/react-query";
import { queryClient } from "@/lib/query-client";
import { initializeProfileStorageNamespace } from "@/lib/profileStorage";
import { logStartupTiming } from "@/lib/startupTiming";
import "./styles/index.css";

async function bootstrap() {
  logStartupTiming("frontend entry loaded");
  // Profile namespace IPC cannot return until Tauri setup() finishes.
  // Do not gate first paint on it — scoped storage resolves lazily.
  void initializeProfileStorageNamespace();
  const [{ default: App }, { showMainWindow }] = await Promise.all([
    import("./App"),
    import("@/lib/showMainWindow"),
    import("@/lib/i18n"),
  ]);

  void showMainWindow();

  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <QueryClientProvider client={queryClient}>
        <App />
      </QueryClientProvider>
    </StrictMode>,
  );
}

void bootstrap();
