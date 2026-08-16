import { BrandMark } from "../../components/BrandMark";
import type { AppStatus } from "../../shared/types";
import { ComposerPanel } from "./ComposerPanel";
import { ResultPanel } from "./ResultPanel";
import type { StudioController } from "./useStudioController";

interface WorkspaceScreenProps {
  status: AppStatus;
  studio: StudioController;
  onChangeKey: () => void;
}

export function WorkspaceScreen({
  status,
  studio,
  onChangeKey,
}: WorkspaceScreenProps) {
  return (
    <main className="studio-shell">
      <header className="studio-header" data-tauri-drag-region>
        <BrandMark />
        <button
          className="key-status-button"
          type="button"
          onClick={onChangeKey}
          disabled={studio.busy}
        >
          <span className="openrouter-status-dot" aria-hidden="true" />
          <span className="key-status-copy">
            <strong>OpenRouter connected</strong>
            <small>Click to change key</small>
          </span>
        </button>
      </header>

      <div className="studio-grid">
        <ComposerPanel status={status} studio={studio} />
        <ResultPanel studio={studio} />
      </div>

      {studio.notice && (
        <div className="notice-toast" role="status">
          <span />
          {studio.notice}
        </div>
      )}
    </main>
  );
}
