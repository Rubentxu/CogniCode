export function GraphEmptyState() {
  return (
    <div
      data-testid="graph-empty-state"
      className="flex h-full items-center justify-center p-4"
    >
      <div className="max-w-md text-center">
        <h3 className="text-sm font-semibold">No call relationships</h3>
        <p className="mt-2 text-xs">
          This object has no incoming or outgoing calls.
        </p>
        <p className="mt-1 text-xs">
          Try a different view like "Overview" or "Source".
        </p>
      </div>
    </div>
  );
}