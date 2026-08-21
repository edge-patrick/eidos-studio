import { useEffect, useMemo, useRef } from "react";
import { createPortal } from "react-dom";
import { CloseIcon } from "../../components/Icons";
import type { ImageModel } from "../../shared/types";
import { ModelIcon } from "./ModelIcon";

interface ModelManagerModalProps {
  models: ImageModel[];
  selectedModelId: string;
  hiddenModelIds: ReadonlySet<string>;
  onVisibilityChange: (modelId: string, visible: boolean) => void;
  onClose: () => void;
}

const estimatedSquare1KPrices: Record<string, string> = {
  "google/gemini-3.1-flash-lite-image": "$0.03",
  "google/gemini-3.1-flash-image": "$0.07",
  "google/gemini-3-pro-image": "$0.13",
  "openai/gpt-image-2": "$0.03",
  "black-forest-labs/flux.2-klein-4b": "$0.014",
  "black-forest-labs/flux.2-pro": "$0.03",
  "black-forest-labs/flux.2-flex": "$0.06",
  "black-forest-labs/flux.2-max": "$0.07",
};

export function ModelManagerModal({
  models,
  selectedModelId,
  hiddenModelIds,
  onVisibilityChange,
  onClose,
}: ModelManagerModalProps) {
  const dialogRef = useRef<HTMLDivElement>(null);
  const closeButtonRef = useRef<HTMLButtonElement>(null);
  const providerGroups = useMemo(() => groupModelsByProvider(models), [models]);

  useEffect(() => {
    const previouslyFocused = document.activeElement;
    const appRoot = document.getElementById("root");
    const appWasInert = appRoot?.hasAttribute("inert") ?? false;
    appRoot?.setAttribute("inert", "");
    closeButtonRef.current?.focus();

    function handleKeyDown(event: globalThis.KeyboardEvent) {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopImmediatePropagation();
        onClose();
        return;
      }
      if (event.key !== "Tab") return;

      const focusable = dialogRef.current?.querySelectorAll<HTMLElement>(
        'button:not([disabled]), input:not([disabled]), [tabindex]:not([tabindex="-1"])',
      );
      if (!focusable || focusable.length === 0) return;

      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    }

    document.addEventListener("keydown", handleKeyDown, true);
    return () => {
      document.removeEventListener("keydown", handleKeyDown, true);
      if (!appWasInert) appRoot?.removeAttribute("inert");
      if (previouslyFocused instanceof HTMLElement) previouslyFocused.focus();
    };
  }, [onClose]);

  return createPortal(
    <div
      className="model-manager-backdrop"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div
        ref={dialogRef}
        className="model-manager-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="model-manager-title"
        aria-describedby="model-manager-description"
      >
        <header className="model-manager-header">
          <div>
            <p className="eyebrow">Model catalog</p>
            <h2 id="model-manager-title">Manage models</h2>
            <p id="model-manager-description">
              Choose which models appear in the model picker.
            </p>
          </div>
          <button
            ref={closeButtonRef}
            className="model-manager-close"
            type="button"
            onClick={onClose}
            aria-label="Close model manager"
          >
            <CloseIcon />
          </button>
        </header>

        <div className="model-manager-content">
          {providerGroups.map(([provider, providerModels]) => (
            <section className="model-provider-group" key={provider}>
              <h3>{provider}</h3>
              <div className="model-manager-list">
                {providerModels.map((model) => {
                  const selected = model.id === selectedModelId;
                  const visible = selected || !hiddenModelIds.has(model.id);
                  const unavailable = !model.available;
                  return (
                    <article
                      className={`model-manager-row${unavailable ? " unavailable" : ""}`}
                      key={model.id}
                    >
                      <ModelIcon modelId={model.id} />
                      <div className="model-manager-copy">
                        <strong>{model.name}</strong>
                        <p>{model.description}</p>
                        <small
                          title="Estimated output price only. Prompts and reference images may add cost."
                        >
                          {unavailable
                            ? model.unavailableReason ?? "Currently unavailable on OpenRouter."
                            : modelSummary(model)}
                        </small>
                      </div>
                      <label
                        className={`model-visibility-toggle${selected ? " selected" : ""}`}
                        title={selected ? "The selected model must remain shown." : undefined}
                      >
                        <span>Show</span>
                        <input
                          type="checkbox"
                          checked={visible}
                          disabled={selected}
                          onChange={(event) =>
                            onVisibilityChange(model.id, event.currentTarget.checked)
                          }
                          aria-label={`Show ${model.name} in model picker`}
                        />
                        <i aria-hidden="true" />
                      </label>
                    </article>
                  );
                })}
              </div>
            </section>
          ))}
        </div>
      </div>
    </div>,
    document.body,
  );
}

function groupModelsByProvider(models: ImageModel[]) {
  const groups = new Map<string, ImageModel[]>();
  for (const model of models) {
    const group = groups.get(model.provider) ?? [];
    group.push(model);
    groups.set(model.provider, group);
  }
  return [...groups.entries()];
}

function modelSummary(model: ImageModel) {
  const price = estimatedSquare1KPrices[model.id];
  const estimate = price
    ? `Est. 1K square: ≈ ${price}`
    : "Est. 1K square: unavailable";
  return `${estimate} · ${formatResolutionRange(model.supportedResolutions)} · ${formatReferenceLimit(model.maxReferences)}`;
}

function formatResolutionRange(resolutions: string[]) {
  if (resolutions.length === 0) return "Provider-selected size";
  if (resolutions.length === 1) return `${resolutions[0]} only`;
  return `${resolutions[0]}–${resolutions[resolutions.length - 1]}`;
}

function formatReferenceLimit(maxReferences: number) {
  if (maxReferences === 0) return "No reference images";
  if (maxReferences === 1) return "Up to 1 reference image";
  return `Up to ${maxReferences} reference images`;
}
