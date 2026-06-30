/**
 * Toast — a simple toast notification component.
 *
 * Auto-dismisses after 6 seconds. Uses raw CSS variables for v1
 * compatibility (no design tokens dependency).
 */
export interface ToastProps {
  message: string;
  onDismiss?: () => void;
}

export function Toast({ message, onDismiss }: ToastProps) {
  return (
    <div
      role="alert"
      style={{
        backgroundColor: "var(--color-surface-overlay, #1e1e2e)",
        color: "var(--color-text-primary, #cdd6f4)",
        padding: "8px 16px",
        borderRadius: 6,
        fontSize: 13,
        boxShadow: "0 2px 8px rgba(0,0,0,0.3)",
        border: "1px solid var(--color-border, #313244)",
        animation: "toast-enter 0.2s ease-out",
      }}
    >
      {message}
    </div>
  );
}
