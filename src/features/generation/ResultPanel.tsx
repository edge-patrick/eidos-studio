import { ErrorMessage } from "../../components/ErrorMessage";
import { ArrowIcon, DownloadIcon } from "../../components/Icons";
import { eidosApi } from "../../shared/eidosApi";
import type { StudioController } from "./useStudioController";

export function ResultPanel({ studio }: { studio: StudioController }) {
  const { generation } = studio;

  return (
    <section className="result-panel" aria-label="Generated image">
      <div className="stage-index" aria-hidden="true">
        <span>OUTPUT</span>
        <span>01 / 01</span>
      </div>

      {generation.status === "generating" ? (
        <DevelopingState onCancel={studio.cancelGeneration} />
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
              className="stage-secondary-button"
              type="button"
              onClick={studio.startNewGeneration}
            >
              New direction
            </button>
            <button
              className="stage-secondary-button"
              type="button"
              onClick={() => void studio.generate()}
            >
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
              Edit direction
            </button>
          </div>
        </div>
      ) : (
        <EmptyState />
      )}
    </section>
  );
}

function DevelopingState({ onCancel }: { onCancel: () => Promise<void> }) {
  return (
    <div className="developing-state">
      <div className="developing-orbit" aria-hidden="true">
        <span />
        <span />
        <span />
      </div>
      <p className="eyebrow">Nano Banana is working</p>
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
      <div className="aperture-mark" aria-hidden="true">
        <span />
        <span />
        <span />
      </div>
      <p className="eyebrow">Ready for exposure</p>
      <h2>Your image appears here.</h2>
      <p>Write a direction, add a reference if useful, then generate.</p>
    </div>
  );
}
