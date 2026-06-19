/**
 * `LandingSuggestionStrip` — shows suggested questions below the graph.
 *
 * Displays `suggested_questions` from the backend and allows the user
 * to click on a question to trigger an ask action.
 */
export interface LandingSuggestionStripProps {
  suggestedQuestions: string[];
  onAsk: (question: string) => void;
}

export function LandingSuggestionStrip({
  suggestedQuestions,
  onAsk,
}: LandingSuggestionStripProps) {
  if (!suggestedQuestions || suggestedQuestions.length === 0) {
    return null;
  }

  return (
    <div
      data-testid="landing-suggestion-strip"
      style={{
        padding: "12px 16px",
        backgroundColor: "var(--color-surface-raised)",
        borderTop: "1px solid var(--color-border)",
      }}
    >
      <div
        style={{
          fontSize: 11,
          color: "var(--color-text-muted)",
          marginBottom: 8,
          fontWeight: 600,
          textTransform: "uppercase",
          letterSpacing: "0.05em",
        }}
      >
        Try asking
      </div>
      <div style={{ display: "flex", flexWrap: "wrap", gap: 8 }}>
        {suggestedQuestions.map((question, i) => (
          <button
            key={i}
            type="button"
            onClick={() => onAsk(question)}
            data-testid={`suggested-question-${i}`}
            style={{
              padding: "6px 12px",
              borderRadius: 16,
              border: "1px solid var(--color-border)",
              backgroundColor: "var(--color-surface-overlay)",
              color: "var(--color-text-primary)",
              fontSize: 12,
              cursor: "pointer",
              transition: "background-color 0.15s, border-color 0.15s",
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.backgroundColor = "var(--color-surface)";
              e.currentTarget.style.borderColor = "var(--color-text-muted)";
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.backgroundColor = "var(--color-surface-overlay)";
              e.currentTarget.style.borderColor = "var(--color-border)";
            }}
          >
            {question}
          </button>
        ))}
      </div>
    </div>
  );
}
