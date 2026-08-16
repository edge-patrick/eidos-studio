import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import nanoBananaLogo from "../../assets/nano-banana-logo.svg";
import {
  ArrowIcon,
  ClockIcon,
  CloseIcon,
  PlusIcon,
} from "../../components/Icons";
import { eidosApi } from "../../shared/eidosApi";
import type { AppStatus } from "../../shared/types";
import type { StudioController } from "./useStudioController";

interface ComposerPanelProps {
  status: AppStatus;
  studio: StudioController;
}

const upcomingModels = [
  { name: "GPT Image", maker: "OpenAI", monogram: "G" },
  { name: "FLUX", maker: "Black Forest Labs", monogram: "F" },
  { name: "Ideogram", maker: "Ideogram", monogram: "I" },
];

export function ComposerPanel({ status, studio }: ComposerPanelProps) {
  const [modelPickerOpen, setModelPickerOpen] = useState(false);
  const modelPickerRef = useRef<HTMLDetailsElement>(null);
  const modelPickerSummaryRef = useRef<HTMLElement>(null);

  function dismissModelPicker(restoreFocus = false) {
    if (modelPickerRef.current) modelPickerRef.current.open = false;
    setModelPickerOpen(false);
    if (restoreFocus) modelPickerSummaryRef.current?.focus();
  }

  useEffect(() => {
    function handlePointerDown(event: PointerEvent) {
      const picker = modelPickerRef.current;
      if (
        picker?.open &&
        event.target instanceof Node &&
        !picker.contains(event.target)
      ) {
        picker.open = false;
        setModelPickerOpen(false);
      }
    }

    function handleKeyDown(event: globalThis.KeyboardEvent) {
      if (event.key !== "Escape" || !modelPickerRef.current?.open) return;
      event.preventDefault();
      modelPickerRef.current.open = false;
      setModelPickerOpen(false);
      modelPickerSummaryRef.current?.focus();
    }

    document.addEventListener("pointerdown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("pointerdown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, []);

  function generate() {
    dismissModelPicker();
    void studio.generate();
  }

  function closeModelPicker() {
    dismissModelPicker(true);
  }

  function handlePromptKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
      event.preventDefault();
      generate();
    }
  }

  return (
    <section className="composer-panel" aria-label="Image prompt">
      <div className="composer-section prompt-section">
        <div className="section-heading">
          <span>01</span>
          <label htmlFor="prompt">Prompt</label>
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

      <div className="composer-section model-section">
        <div className="section-heading">
          <span>02</span>
          <span>Model</span>
        </div>

        <details
          ref={modelPickerRef}
          className={`model-picker${studio.busy ? " disabled" : ""}`}
          onClick={(event) => {
            if (studio.busy) event.preventDefault();
          }}
          onToggle={(event) => {
            setModelPickerOpen(event.currentTarget.open);
          }}
        >
          <summary
            ref={modelPickerSummaryRef}
            aria-haspopup="listbox"
            aria-label="Select image model"
            aria-expanded={modelPickerOpen}
            aria-controls="image-model-options"
          >
            <span className="model-icon banana-icon">
              <img src={nanoBananaLogo} alt="" draggable={false} />
            </span>
            <span className="model-copy">
              <strong>{status.modelName}</strong>
              <small>{status.modelId}</small>
            </span>
            <span className="model-chevron" aria-hidden="true" />
          </summary>

          <div
            id="image-model-options"
            className="model-menu"
            role="listbox"
            aria-label="Image model"
          >
            <button
              className="model-option selected"
              type="button"
              role="option"
              aria-selected="true"
              onClick={closeModelPicker}
            >
              <span className="model-icon banana-icon">
                <img src={nanoBananaLogo} alt="" draggable={false} />
              </span>
              <span className="model-copy">
                <strong>{status.modelName}</strong>
                <small>{status.modelId}</small>
              </span>
              <span className="model-check" aria-hidden="true">✓</span>
            </button>

            {upcomingModels.map((model) => (
              <button
                className="model-option"
                key={model.name}
                type="button"
                role="option"
                aria-selected="false"
                disabled
              >
                <span className="model-icon model-monogram" aria-hidden="true">
                  {model.monogram}
                </span>
                <span className="model-copy">
                  <strong>{model.name}</strong>
                  <small>{model.maker}</small>
                </span>
                <span className="coming-soon">
                  <ClockIcon />
                  Coming soon
                </span>
              </button>
            ))}
          </div>
        </details>
      </div>

      <div className="composer-section reference-section">
        <div className="section-heading">
          <span>03</span>
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
            <span className="reference-plus" aria-hidden="true">
              <PlusIcon />
            </span>
            <div className="reference-copy">
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
          <span>04</span>
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
          onClick={generate}
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
