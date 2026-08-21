import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type KeyboardEvent,
} from "react";
import {
  ArrowIcon,
  CloseIcon,
  PlusIcon,
} from "../../components/Icons";
import { eidosApi } from "../../shared/eidosApi";
import type { AppStatus, ReferenceSelection } from "../../shared/types";
import { ModelIcon } from "./ModelIcon";
import { ModelManagerModal } from "./ModelManagerModal";
import type { StudioController } from "./useStudioController";

interface ComposerPanelProps {
  status: AppStatus;
  studio: StudioController;
}

const HIDDEN_MODELS_STORAGE_KEY = "eidos.hidden-image-models.v1";

export function ComposerPanel({ status, studio }: ComposerPanelProps) {
  const [modelPickerOpen, setModelPickerOpen] = useState(false);
  const [modelManagerOpen, setModelManagerOpen] = useState(false);
  const [hiddenModelIds, setHiddenModelIds] = useState<Set<string>>(
    readHiddenModelIds,
  );
  const [draggedReference, setDraggedReference] = useState<string | null>(null);
  const modelPickerRef = useRef<HTMLDetailsElement>(null);
  const modelPickerSummaryRef = useRef<HTMLElement>(null);
  const modelManageButtonRef = useRef<HTMLButtonElement>(null);

  function dismissModelPicker(restoreFocus = false) {
    if (modelPickerRef.current) modelPickerRef.current.open = false;
    setModelPickerOpen(false);
    if (restoreFocus) modelPickerSummaryRef.current?.focus();
  }

  const closeModelManager = useCallback(() => {
    setModelManagerOpen(false);
    window.setTimeout(() => modelManageButtonRef.current?.focus(), 0);
  }, []);

  function setModelVisible(modelId: string, visible: boolean) {
    if (!visible && modelId === studio.selectedModelId) return;
    setHiddenModelIds((current) => {
      const next = new Set(current);
      if (visible) next.delete(modelId);
      else next.add(modelId);
      persistHiddenModelIds(next);
      return next;
    });
  }

  const visibleModels = studio.models.filter(
    (model) =>
      model.id === studio.selectedModelId || !hiddenModelIds.has(model.id),
  );

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

  useEffect(() => {
    if (!hiddenModelIds.has(studio.selectedModelId)) return;
    setHiddenModelIds((current) => {
      const next = new Set(current);
      next.delete(studio.selectedModelId);
      persistHiddenModelIds(next);
      return next;
    });
  }, [hiddenModelIds, studio.selectedModelId]);

  function generate() {
    dismissModelPicker();
    void studio.generate();
  }

  function chooseModel(modelId: string) {
    studio.selectModel(modelId);
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
          <button
            ref={modelManageButtonRef}
            className="model-manage-button"
            type="button"
            onClick={() => {
              dismissModelPicker();
              setModelManagerOpen(true);
            }}
            disabled={studio.busy}
          >
            Manage
          </button>
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
            <ModelIcon modelId={studio.selectedModelId} />
            <span className="model-copy">
              <strong>{studio.selectedModel.name}</strong>
              <small>{studio.selectedModel.id}</small>
            </span>
            <span className="model-chevron" aria-hidden="true" />
          </summary>

          <div
            id="image-model-options"
            className="model-menu"
            role="listbox"
            aria-label="Image model"
          >
            {visibleModels.map((model) => (
              <button
                className={`model-option${model.id === studio.selectedModelId ? " selected" : ""}`}
                key={model.id}
                type="button"
                role="option"
                aria-selected={model.id === studio.selectedModelId}
                onClick={() => chooseModel(model.id)}
                disabled={studio.busy || !model.available}
                title={model.available ? model.description : model.unavailableReason}
              >
                <ModelIcon modelId={model.id} />
                <span className="model-copy">
                  <strong>{model.name}</strong>
                  <small>
                    {model.available
                      ? `${model.provider} · ${formatResolutionRange(model.supportedResolutions)}`
                      : model.unavailableReason ?? "Unavailable on OpenRouter. Try again later."}
                  </small>
                </span>
                {model.id === studio.selectedModelId && (
                  <span className="model-check" aria-hidden="true">✓</span>
                )}
              </button>
            ))}
          </div>
        </details>
      </div>

      <div className="composer-section reference-section">
        <div className="section-heading">
          <span>03</span>
          <span>
            References{" "}
            {studio.maxReferences > 0 && (
              <span className="section-title-qualifier">(Optional)</span>
            )}
          </span>
          {studio.maxReferences === 0 ? (
            <small>Not supported</small>
          ) : studio.references.length > 0 ? (
            <small>{studio.references.length} / {studio.maxReferences}</small>
          ) : null}
        </div>

        {studio.maxReferences === 0 && (
          <p className="reference-support-note" role="status">
            {studio.selectedModel.name} does not support reference images.
            {studio.references.length > 0 &&
              " Remove the selected images or choose another model."}
          </p>
        )}

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

        {studio.referenceLimitExceeded && studio.maxReferences > 0 && (
          <p className="reference-limit-warning" role="alert">
            {studio.selectedModel.name} accepts {studio.maxReferences} references. Remove the
            extras or choose another model.
          </p>
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
          {studio.selectedModel.supportedResolutions.length > 0 ? (
            <OptionStrip
              id="resolution"
              className="resolution-options"
              label="Resolution"
              options={studio.resolutionOptions}
              value={studio.resolution}
              onChange={studio.setResolution}
              disabled={studio.busy}
            />
          ) : (
            <p className="resolution-support-note" role="status">
              {studio.selectedModel.name} uses the provider&apos;s default output size.
            </p>
          )}
        </div>
      </div>

      <div className="composer-footer">
        <div className="generation-summary">
          <span>Output</span>
          <strong>
            1 image · {formatOption(studio.aspectRatio)}
            {studio.selectedModel.supportedResolutions.length > 0 && (
              <> · {formatOption(studio.resolution)}</>
            )}
          </strong>
        </div>
        <button
          className="primary-button generate-button"
          type="button"
          onClick={generate}
          disabled={
            !status.hasApiKey ||
            !studio.selectedModel.available ||
            !studio.prompt.trim() ||
            studio.referenceBusy ||
            studio.referenceLimitExceeded ||
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

      {modelManagerOpen && (
        <ModelManagerModal
          models={studio.models}
          selectedModelId={studio.selectedModelId}
          hiddenModelIds={hiddenModelIds}
          onVisibilityChange={setModelVisible}
          onClose={closeModelManager}
        />
      )}
    </section>
  );
}

function readHiddenModelIds() {
  try {
    const stored = window.localStorage.getItem(HIDDEN_MODELS_STORAGE_KEY);
    if (!stored) return new Set<string>();
    const parsed: unknown = JSON.parse(stored);
    if (!Array.isArray(parsed)) return new Set<string>();
    return new Set(parsed.filter((value): value is string => typeof value === "string"));
  } catch {
    return new Set<string>();
  }
}

function persistHiddenModelIds(modelIds: ReadonlySet<string>) {
  try {
    window.localStorage.setItem(
      HIDDEN_MODELS_STORAGE_KEY,
      JSON.stringify([...modelIds]),
    );
  } catch {
    // Model visibility remains available for the current session.
  }
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

function formatResolutionRange(resolutions: string[]) {
  if (resolutions.length === 0) return "Provider default";
  if (resolutions.length === 1) return `${resolutions[0]} only`;
  return `${resolutions[0]}–${resolutions[resolutions.length - 1]}`;
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
        style={{ gridTemplateColumns: `repeat(${options.length}, minmax(0, 1fr))` }}
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
