import type { AppError } from "../shared/types";

export function ErrorMessage({
  error,
  compact = false,
}: {
  error: AppError;
  compact?: boolean;
}) {
  return (
    <div className={`error-message ${compact ? "compact" : ""}`} role="alert">
      <p>{error.message}</p>
      {error.details && (
        <details>
          <summary>Technical details</summary>
          <code>{error.details}</code>
        </details>
      )}
    </div>
  );
}
