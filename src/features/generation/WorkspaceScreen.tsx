import { BrandMark } from "../../components/BrandMark";
import { KeyIcon } from "../../components/Icons";
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
      <header className="studio-header">
        <BrandMark />
        <div className="header-center" aria-label="Active model">
          <span className="connection-dot" />
          <span>{status.modelName}</span>
          <span className="model-separator">/</span>
          <span>{status.modelId}</span>
        </div>
        <button
          className="key-status-button"
          type="button"
          onClick={onChangeKey}
          disabled={studio.busy}
        >
          <KeyIcon />
          Key connected
        </button>
      </header>

      <div className="studio-grid">
        <ComposerPanel studio={studio} />
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
