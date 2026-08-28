import { Component, type ErrorInfo, type ReactNode, useEffect } from "react";
import i18next from "i18next";
import Layout from "./app/Layout";
import { scheduleAppSplashDismiss } from "@/lib/splash";
import { logStartupTiming } from "@/lib/startupTiming";

class ErrorBoundary extends Component<
  { children: ReactNode },
  { error: Error | null }
> {
  state: { error: Error | null } = { error: null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("[ErrorBoundary]", error, info.componentStack);
  }

  render() {
    if (this.state.error) {
      return (
        <div style={{
          display: "flex", flexDirection: "column", alignItems: "center",
          justifyContent: "center", height: "100vh", gap: 16, padding: 24,
          fontFamily: "-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
          color: "var(--color-text-primary, #333)",
          backgroundColor: "var(--color-bg, #fff)",
        }}>
          <h2 style={{ margin: 0, fontSize: 18, fontWeight: 600 }}>{i18next.t("errorBoundary.title", "Something went wrong")}</h2>
          <p style={{ margin: 0, fontSize: 14, color: "var(--color-text-secondary, #666)" }}>
            {i18next.t("errorBoundary.description", "Please try refreshing the application.")}
          </p>
          <button
            onClick={() => window.location.reload()}
            style={{
              marginTop: 8, padding: "8px 20px", cursor: "pointer",
              backgroundColor: "var(--color-accent, #2563eb)", color: "#fff",
              border: "none", borderRadius: 6, fontSize: 14, fontWeight: 600,
            }}
          >
            {i18next.t("errorBoundary.refresh", "Refresh")}
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}

export default function App() {
  useEffect(() => {
    logStartupTiming("react app mounted");
    scheduleAppSplashDismiss();
  }, []);

  return (
    <ErrorBoundary>
      <Layout />
    </ErrorBoundary>
  );
}
