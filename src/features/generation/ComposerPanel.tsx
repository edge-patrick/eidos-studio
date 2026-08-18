import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import nanoBananaLogo from "../../assets/nano-banana-logo.svg";
import {
  ArrowIcon,
  ClockIcon,
  CloseIcon,
  PlusIcon,
} from "../../components/Icons";
import { eidosApi } from "../../shared/eidosApi";
import type { AppStatus, ReferenceSelection } from "../../shared/types";
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
  const [draggedReference, setDraggedReference] = useState<string | null>(null);
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
    if (
      status.hasApiKey &&
      event.key === "Enter" &&
      (event.metaKey || event.ctrlKey)
    ) {
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
          <span>References</span>
          <small>
            {studio.references.length > 0
              ? `${studio.references.length} / ${studio.maxReferences}`
              : "Optional"}
          </small>
        </div>

        {studio.references.length > 0 && (
          <div
            className="reference-list"
            role="list"
            aria-label="Selected reference images"
          >
            {studio.references.map((reference, index) => (
              <div
                className={`reference-card${draggedReference === reference.token ? " dragging" : ""}`}
                key={reference.token}
                role="listitem"
                draggable={!studio.busy}
                onDragStart={(event) => {
                  setDraggedReference(reference.token);
                  event.dataTransfer.effectAllowed = "move";
                  event.dataTransfer.setData("text/plain", reference.token);
                }}
                onDragEnd={() => setDraggedReference(null)}
                onDragOver={(event) => {
                  if (!studio.busy) event.preventDefault();
                }}
                onDrop={(event) => {
                  event.preventDefault();
                  const sourceToken =
                    draggedReference || event.dataTransfer.getData("text/plain");
                  studio.moveReference(sourceToken, reference.token);
                  setDraggedReference(null);
                }}
              >
                <span className="reference-order" aria-hidden="true">
                  {index + 1}
                </span>
                <ReferenceThumbnail reference={reference} index={index} />
                <div className="reference-info">
                  <strong>{reference.fileName}</strong>
                  <span>
                    {reference.width} × {reference.height}
                  </span>
                </div>
                <div className="reference-actions">
                  <button
                    type="button"
                    onClick={() => studio.shiftReference(reference.token, -1)}
                    disabled={studio.busy || index === 0}
                    aria-label={`Move ${reference.fileName} earlier`}
                  >
                    <span aria-hidden="true">↑</span>
                  </button>
                  <button
                    type="button"
                    onClick={() => studio.shiftReference(reference.token, 1)}
                    disabled={studio.busy || index === studio.references.length - 1}
                    aria-label={`Move ${reference.fileName} later`}
                  >
                    <span aria-hidden="true">↓</span>
                  </button>
                  <button
                    className="remove-reference"
                    type="button"
                    onClick={() => void studio.removeReference(reference.token)}
                    disabled={studio.busy}
                    aria-label={`Remove ${reference.fileName}`}
                  >
                    <CloseIcon />
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}

        {studio.references.length < studio.maxReferences &&
          studio.referenceTotalBytes < studio.maxReferenceTotalBytes && (
          <button
            className={`reference-picker${studio.references.length > 0 ? " compact" : ""}`}
            type="button"
            onClick={() => void studio.chooseReferences()}
            disabled={studio.referenceBusy || studio.busy}
          >
            <span className="reference-plus" aria-hidden="true">
              <PlusIcon />
            </span>
            <div className="reference-copy">
              <strong>
                {studio.referenceBusy
                  ? "Opening files…"
                  : studio.references.length > 0
                    ? "Add more images"
                    : "Add reference images"}
              </strong>
              <span>
                PNG, JPEG or WebP · 12 MB each ·{" "}
                {Math.round(studio.maxReferenceTotalBytes / 1024 / 1024)} MB total
              </span>
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
          disabled={
            !status.hasApiKey ||
            !studio.prompt.trim() ||
            studio.referenceBusy ||
            studio.busy
          }
          title={
            status.hasApiKey
              ? undefined
              : "Connect OpenRouter before generating"
          }
        >
          <span>Generate image</span>
          <ArrowIcon />
        </button>
      </div>
    </section>
  );
}

function ReferenceThumbnail({
  reference,
  index,
}: {
  reference: ReferenceSelection;
  index: number;
}) {
  const [sourcePath, setSourcePath] = useState(reference.thumbnailPath);

  useEffect(() => {
    setSourcePath(reference.thumbnailPath);
  }, [reference.thumbnailPath]);

  return (
    <img
      src={eidosApi.assetUrl(sourcePath)}
      alt={`Reference ${index + 1}`}
      loading="lazy"
      decoding="async"
      draggable={false}
      onError={() => {
        if (sourcePath !== reference.assetPath) {
          setSourcePath(reference.assetPath);
        }
      }}
    />
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
