/**
 * Application entrypoint.
 *
 * Starts the MSW browser worker (when `VITE_USE_MOCKS=true`) before
 * mounting the React tree. The flag is opt-in so production builds
 * never ship the worker.
 */
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./tailwind.css";
import App from "./App";

async function bootstrap() {
  const useMocks = import.meta.env.VITE_USE_MOCKS === "true";
  if (useMocks) {
    const { worker } = await import("./mocks/browser");
    await worker.start({
      onUnhandledRequest: "bypass",
      serviceWorker: { url: "/mockServiceWorker.js" },
    });
  }

  const rootEl = document.getElementById("root");
  if (!rootEl) {
    throw new Error("Root element #root not found in index.html");
  }

  createRoot(rootEl).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}

void bootstrap();
