/**
 * NotificationProvider — React context for showing toast notifications.
 *
 * Provides a simple `showNotification(message: string)` method for triggering
 * toast messages to confirm actions (e.g., "Copied to clipboard").
 */
import {
  createContext,
  useCallback,
  useState,
  type ReactNode,
} from "react";

export interface Toast {
  id: string;
  message: string;
}

interface NotificationContextValue {
  showNotification: (message: string) => void;
}

export const NotificationContext = createContext<NotificationContextValue>({
  showNotification: () => {
    // Default no-op for when context is not yet provided
  },
});

export function NotificationProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);

  const showNotification = useCallback((message: string) => {
    const id = `toast-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
    setToasts((prev) => [...prev, { id, message }]);

    // Auto-dismiss after 3 seconds
    setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, 3000);
  }, []);

  return (
    <NotificationContext.Provider value={{ showNotification }}>
      {children}
      {/* Toast container rendered at provider level */}
      <div
        style={{
          position: "fixed",
          bottom: 16,
          right: 16,
          display: "flex",
          flexDirection: "column",
          gap: 8,
          zIndex: 9999,
          pointerEvents: "none",
        }}
      >
        {toasts.map((toast) => (
          <div
            key={toast.id}
            style={{
              backgroundColor: "var(--color-surface-overlay, #1e1e2e)",
              color: "var(--color-text-primary, #cdd6f4)",
              padding: "8px 16px",
              borderRadius: 6,
              fontSize: 13,
              boxShadow: "0 2px 8px rgba(0,0,0,0.3)",
              border: "1px solid var(--color-border, #313244)",
            }}
          >
            {toast.message}
          </div>
        ))}
      </div>
    </NotificationContext.Provider>
  );
}
