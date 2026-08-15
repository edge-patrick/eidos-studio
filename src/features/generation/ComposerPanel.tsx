import type { KeyboardEvent } from "react";
import { ArrowIcon, CloseIcon } from "../../components/Icons";
import { eidosApi } from "../../shared/eidosApi";
import type { StudioController } from "./useStudioController";

export function ComposerPanel({ studio }: { studio: StudioController }) {
  function handlePromptKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
      event.preventDefault();
      void studio.generate();
    }
  }

  return (
    <section className="composer-panel" aria-label="Image direction">
      <div className="composer-section prompt-section">
        <div className="section-heading">
          <span>01</span>
          <label htmlFor="prompt">Direction</label>
          {studio.prompt.length > 6500 && (
            <small>{studio.prompt.length} / 8000</small>
          )}
        </div>
        <textarea
          ref={studio.promptRef}
          id="prompt"
          value={studio.prompt}
          onChange={(event) => studio.setPrompt(event.currentTarget.value)}
          onKeyDown={handlePromptKeyDown}
          placeholder="Describe the image you want to make…"
          maxLength={8000}
          disabled={studio.busy}
          autoFocus
        />
        <p className="prompt-hint">
          <span>⌘ ↵</span> to generate
        </p>
      </div>

      <div className="composer-section reference-section">
        <div className="section-heading">
          <span>02</span>
          <span>Reference</span>
          <small>Optional</small>
        </div>

        {studio.reference ? (
          <div className="reference-card">
            <img
              src={eidosApi.assetUrl(studio.reference.assetPath)}
              alt="Selected reference"
            />
            <div className="reference-info">
              <strong>{studio.reference.fileName}</strong>
              <span>
                {studio.reference.width} × {studio.reference.height}
              </span>
            </div>
            <button
              className="remove-reference"
              type="button"
              onClick={() => void studio.removeReference()}
              disabled={studio.busy}
              aria-label="Remove reference image"
            >
              <CloseIcon />
            </button>
          </div>
        ) : (
          <button
            className="reference-picker"
            type="button"
            onClick={() => void studio.chooseReference()}
            disabled={studio.referenceBusy || studio.busy}
          >
            <div className="reference-plus">+</div>
            <div>
              <strong>
                {studio.referenceBusy ? "Opening files…" : "Add an image"}
              </strong>
              <span>PNG, JPEG or WebP · 12 MB max</span>
            </div>
          </button>
        )}
      </div>

      <div className="composer-section settings-section">
        <div className="section-heading">
          <span>03</span>
          <span>Size</span>
        </div>

        <div className="settings-grid">
          <OptionStrip
            id="aspect-ratio"
            className="ratio-options"
            label="Ratio"
            options={studio.aspectRatioOptions}
            value={studio.aspectRatio}
            onChange={studio.setAspectRatio}
            disabled={studio.busy}
          />
          <OptionStrip
            id="resolution"
            className="resolution-options"
            label="Resolution"
            options={studio.resolutionOptions}
            value={studio.resolution}
            onChange={studio.setResolution}
            disabled={studio.busy}
          />
        </div>
      </div>

      <div className="composer-footer">
        <div className="generation-summary">
          <span>Output</span>
          <strong>
            1 image · {formatOption(studio.aspectRatio)} ·{" "}
            {formatOption(studio.resolution)}
          </strong>
        </div>
        <button
          className="primary-button generate-button"
          type="button"
          onClick={() => void studio.generate()}
          disabled={!studio.prompt.trim() || studio.busy}
        >
          <span>Generate image</span>
          <ArrowIcon />
        </button>
      </div>
    </section>
  );
}

interface OptionStripProps {
  id: string;
  className: string;
  label: string;
  options: string[];
  value: string;
  onChange: (value: string) => void;
  disabled: boolean;
}

function OptionStrip({
  id,
  className,
  label,
  options,
  value,
  onChange,
  disabled,
}: OptionStripProps) {
  const labelId = `${id}-label`;
  return (
    <div className="settings-control">
      <span id={labelId}>{label}</span>
      <div
        className={`option-strip ${className}`}
        role="group"
        aria-labelledby={labelId}
      >
        {options.map((option) => (
          <button
            key={option}
            type="button"
            className={value === option ? "selected" : undefined}
            aria-pressed={value === option}
            onClick={() => onChange(option)}
            disabled={disabled}
          >
            {formatOption(option)}
          </button>
        ))}
      </div>
    </div>
  );
}

function formatOption(option: string) {
  return option === "auto" ? "Auto" : option;
}
