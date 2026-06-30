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
import { Toast } from "./Toast";

export interface ToastItem {
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
  const [toasts, setToasts] = useState<ToastItem[]>([]);

  const showNotification = useCallback((message: string) => {
    const id = `toast-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
    setToasts((prev) => [...prev, { id, message }]);

    // Auto-dismiss after 6 seconds
    setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, 6000);
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
          <Toast key={toast.id} message={toast.message} />
        ))}
      </div>
    </NotificationContext.Provider>
  );
}
