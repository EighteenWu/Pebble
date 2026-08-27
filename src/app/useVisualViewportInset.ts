import { useEffect } from "react";
import { isAndroidRuntime } from "@/lib/platform";

/** Keep the Android shell above the software keyboard. */
export function useVisualViewportInset() {
  useEffect(() => {
    if (!isAndroidRuntime() || !window.visualViewport) {
      return;
    }

    const viewport = window.visualViewport;
    const update = () => {
      const inset = Math.max(0, window.innerHeight - viewport.height - viewport.offsetTop);
      document.documentElement.style.setProperty("--keyboard-inset", `${inset}px`);
    };

    update();
    viewport.addEventListener("resize", update);
    viewport.addEventListener("scroll", update);
    return () => {
      viewport.removeEventListener("resize", update);
      viewport.removeEventListener("scroll", update);
      document.documentElement.style.removeProperty("--keyboard-inset");
    };
  }, []);
}
