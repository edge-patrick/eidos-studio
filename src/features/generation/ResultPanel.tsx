import eidosLogo from "../../assets/eidos-logo.svg";
import { ErrorMessage } from "../../components/ErrorMessage";
import {
  ArrowIcon,
  DownloadIcon,
  PlusIcon,
  RetryIcon,
} from "../../components/Icons";
import { eidosApi } from "../../shared/eidosApi";
import type { StudioController } from "./useStudioController";

export function ResultPanel({ studio }: { studio: StudioController }) {
  const { generation } = studio;

  return (
    <section className="result-panel" aria-label="Generated image">
      {generation.status === "generating" ? (
        <DevelopingState
          modelName={studio.selectedModel.name}
          onCancel={studio.cancelGeneration}
        />
      ) : generation.status === "ready" ? (
        <div className="result-view">
          <div className="result-image-wrap">
            <img
              src={eidosApi.assetUrl(generation.result.assetPath)}
              alt="Generated result"
            />
          </div>
          <div className="result-meta">
            <div>
              <span>{generation.result.providerName ?? "OpenRouter route"}</span>
              <strong>
                {generation.result.width} × {generation.result.height}
              </strong>
            </div>
            <div>
              <span>Time</span>
              <strong>{(generation.result.durationMs / 1000).toFixed(1)}s</strong>
            </div>
            <div>
              <span>Cost</span>
              <strong>
                {generation.result.costUsd === undefined
                  ? "—"
                  : `$${generation.result.costUsd.toFixed(4)}`}
              </strong>
            </div>
          </div>
          {generation.actionError && (
            <ErrorMessage error={generation.actionError} compact />
          )}
          <div className="result-actions">
            <button
              className="result-action-button"
              type="button"
              onClick={studio.startNewGeneration}
            >
              <PlusIcon />
              New prompt
            </button>
            <button
              className="result-action-button"
              type="button"
              onClick={() => void studio.generate()}
            >
              <RetryIcon />
              Retry
            </button>
            <button
              className="result-save-button"
              type="button"
              onClick={() => void studio.saveResult()}
              disabled={generation.saving}
            >
              {generation.saving ? "Saving…" : "Save as"}
              <DownloadIcon />
            </button>
          </div>
        </div>
      ) : studio.generationError ? (
        <div className="stage-error">
          <span className="error-code">{studio.generationError.kind}</span>
          <h2>Image not made</h2>
          <ErrorMessage error={studio.generationError} />
          <div className="stage-error-actions">
            {studio.generationError.retryable && (
              <button
                className="result-save-button"
                type="button"
                onClick={() => void studio.generate()}
              >
                Retry
                <ArrowIcon />
              </button>
            )}
            <button
              className="stage-secondary-button"
              type="button"
              onClick={studio.startNewGeneration}
            >
              Edit prompt
            </button>
          </div>
        </div>
      ) : (
        <EmptyState />
      )}
    </section>
  );
}

function DevelopingState({
  modelName,
  onCancel,
}: {
  modelName: string;
  onCancel: () => Promise<void>;
}) {
  return (
    <div className="developing-state">
      <div className="developing-logo" aria-hidden="true">
        <img src={eidosLogo} alt="" draggable={false} />
      </div>
      <p className="eyebrow">{modelName} is working</p>
      <h2>Developing image</h2>
      <div className="exposure-line" aria-hidden="true">
        <span />
      </div>
      <p className="developing-note">Some generations take a minute or two.</p>
      <button
        className="stage-secondary-button"
        type="button"
        onClick={() => void onCancel()}
      >
        Cancel generation
      </button>
    </div>
  );
}

function EmptyState() {
  return (
    <div className="empty-stage">
      <div className="empty-logo-mark" aria-hidden="true">
        <img src={eidosLogo} alt="" draggable={false} />
      </div>
      <p className="eyebrow">Ready for exposure</p>
      <h2>Your image appears here.</h2>
      <p>Write a prompt, add references if useful, then generate.</p>
    </div>
  );
}
