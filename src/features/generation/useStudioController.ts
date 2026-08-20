import { useEffect, useReducer, useRef, useState } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { eidosApi, normalizeError } from "../../shared/eidosApi";
import type {
  AppError,
  AppStatus,
  GenerateRequest,
  HistoryAttempt,
  ImageModel,
  ReferenceSelection,
} from "../../shared/types";
import {
  generationReducer,
  initialGenerationState,
} from "./generationState";

interface ModelSettingsDraft {
  aspectRatio: string;
  resolution: string;
}

const defaultSettings: ModelSettingsDraft = {
  aspectRatio: "auto",
  resolution: "auto",
};

export function useStudioController(status: AppStatus, models: ImageModel[]) {
  const [selectedModelId, setSelectedModelId] = useState(status.modelId);
  const selectedModel =
    models.find((model) => model.id === selectedModelId && model.available) ??
    models.find((model) => model.isDefault && model.available) ??
    models.find((model) => model.available) ??
    models.find((model) => model.id === selectedModelId) ??
    models[0] ?? {
      id: status.modelId,
      name: status.modelName,
      provider: "Google",
      description: "Balanced image generation.",
      available: true,
      isDefault: true,
      supportedAspectRatios: status.supportedAspectRatios,
      supportedResolutions: status.supportedResolutions,
      maxReferences: status.maxReferences,
    };
  const maxReferences = selectedModel.maxReferences;
  const maxReferenceTotalBytes = status.maxReferenceTotalBytes;
  const [prompt, setPrompt] = useState("");
  const [references, setReferences] = useState<ReferenceSelection[]>([]);
  const referenceTotalBytes = references.reduce(
    (total, reference) => total + reference.sizeBytes,
    0,
  );
  const [referenceBusy, setReferenceBusy] = useState(false);
  const [settingsByModel, setSettingsByModel] = useState<
    Record<string, ModelSettingsDraft>
  >({});
  const selectedSettings = settingsByModel[selectedModel.id] ?? defaultSettings;
  const aspectRatio =
    selectedSettings.aspectRatio === "auto" ||
    selectedModel.supportedAspectRatios.includes(selectedSettings.aspectRatio)
      ? selectedSettings.aspectRatio
      : "auto";
  const resolution =
    selectedSettings.resolution === "auto" ||
    selectedModel.supportedResolutions.includes(selectedSettings.resolution)
      ? selectedSettings.resolution
      : "auto";
  const [inputError, setInputError] = useState<AppError | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [generation, dispatch] = useReducer(
    generationReducer,
    initialGenerationState,
  );
  const activeRequestId = useRef<string | null>(null);
  const pendingCancellationId = useRef<string | null>(null);
  const eventListenerReady = useRef<Promise<UnlistenFn> | null>(null);
  const promptRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (
      models.length === 0 ||
      models.some((model) => model.id === selectedModelId && model.available)
    ) {
      return;
    }
    const fallback =
      models.find((model) => model.isDefault && model.available) ??
      models.find((model) => model.available);
    if (fallback) setSelectedModelId(fallback.id);
  }, [models, selectedModelId]);

  useEffect(() => {
    const unlisten = eidosApi.listenToGenerationJobs((event) => {
      if (event.requestId !== activeRequestId.current) return;

      if (event.status === "succeeded" && event.result) {
        dispatch({
          type: "succeeded",
          requestId: event.requestId,
          result: event.result,
        });
      } else if (event.status === "cancelled") {
        dispatch({ type: "cancelled", requestId: event.requestId });
        setNotice("Generation cancelled.");
      } else {
        dispatch({
          type: "failed",
          requestId: event.requestId,
          error:
            event.error ??
            normalizeError("Generation finished without a result."),
        });
      }
      activeRequestId.current = null;
      pendingCancellationId.current = null;
    });
    eventListenerReady.current = unlisten;

    return () => {
      eventListenerReady.current = null;
      void unlisten.then((stop) => stop()).catch(() => undefined);
    };
  }, []);

  useEffect(() => {
    if (!notice) return;
    const timeout = window.setTimeout(() => setNotice(null), 5000);
    return () => window.clearTimeout(timeout);
  }, [notice]);

  async function chooseReferences() {
    const selectedBytes = referenceTotalBytes;
    const remainingBytes = Math.max(0, maxReferenceTotalBytes - selectedBytes);
    if (references.length >= maxReferences || remainingBytes === 0) return;
    setReferenceBusy(true);
    setInputError(null);
    try {
      const selections = await eidosApi.selectReferences(selectedBytes);
      if (selections.length === 0) return;
      const available = maxReferences - references.length;
      const accepted = selections.slice(0, available);
      const overflow = selections.slice(available);
      setReferences((current) => [...current, ...accepted]);
      await Promise.all(
        overflow.map((selection) => eidosApi.discardReference(selection.token)),
      );
      if (overflow.length > 0) {
        setNotice(`${selectedModel.name} accepts up to ${maxReferences} reference images.`);
      }
    } catch (error) {
      setInputError(normalizeError(error));
    } finally {
      setReferenceBusy(false);
    }
  }

  async function removeReference(token: string) {
    if (!references.some((reference) => reference.token === token)) return;
    setReferences((current) =>
      current.filter((reference) => reference.token !== token),
    );
    try {
      await eidosApi.discardReference(token);
    } catch (error) {
      setInputError(normalizeError(error));
    }
  }

  function moveReference(sourceToken: string, targetToken: string) {
    if (sourceToken === targetToken) return;
    setReferences((current) => {
      const sourceIndex = current.findIndex(({ token }) => token === sourceToken);
      const targetIndex = current.findIndex(({ token }) => token === targetToken);
      if (sourceIndex < 0 || targetIndex < 0) return current;
      const next = [...current];
      const [source] = next.splice(sourceIndex, 1);
      next.splice(targetIndex, 0, source);
      return next;
    });
  }

  function shiftReference(token: string, offset: -1 | 1) {
    setReferences((current) => {
      const sourceIndex = current.findIndex((reference) => reference.token === token);
      const targetIndex = sourceIndex + offset;
      if (sourceIndex < 0 || targetIndex < 0 || targetIndex >= current.length) {
        return current;
      }
      const next = [...current];
      [next[sourceIndex], next[targetIndex]] = [next[targetIndex], next[sourceIndex]];
      return next;
    });
  }

  function selectModel(modelId: string) {
    if (
      generation.status === "generating" ||
      !models.some(({ id, available }) => id === modelId && available)
    ) {
      return;
    }
    setSelectedModelId(modelId);
    setInputError(null);
    setNotice(null);
  }

  function setModelSetting(
    key: keyof ModelSettingsDraft,
    value: string,
  ) {
    const modelId = selectedModel.id;
    setSettingsByModel((current) => ({
      ...current,
      [modelId]: {
        ...defaultSettings,
        ...current[modelId],
        [key]: value,
      },
    }));
  }

  const referenceLimitExceeded = references.length > maxReferences;

  async function generate() {
    if (
      !status.hasApiKey ||
      !selectedModel.id ||
      !selectedModel.available ||
      !prompt.trim() ||
      referenceBusy ||
      referenceLimitExceeded ||
      generation.status === "generating"
    ) {
      return;
    }
    const requestId = crypto.randomUUID();
    const request: GenerateRequest = {
      requestId,
      modelId: selectedModel.id,
      prompt,
      referenceTokens: references.map(({ token }) => token),
      aspectRatio: aspectRatio === "auto" ? null : aspectRatio,
      resolution: resolution === "auto" ? null : resolution,
    };

    activeRequestId.current = requestId;
    setInputError(null);
    setNotice(null);
    dispatch({ type: "started", requestId });
    try {
      const listener = eventListenerReady.current;
      if (!listener) {
        throw new Error("The generation event listener is not ready.");
      }
      await listener;
      await eidosApi.startGeneration(request);
      if (
        pendingCancellationId.current === requestId &&
        activeRequestId.current === requestId
      ) {
        try {
          const result = await eidosApi.cancelGeneration(requestId);
          if (result.cancelled) pendingCancellationId.current = null;
        } catch (error) {
          setInputError(normalizeError(error));
        }
      }
    } catch (error) {
      activeRequestId.current = null;
      pendingCancellationId.current = null;
      const normalized = normalizeError(error);
      if (normalized.kind === "cancelled") {
        dispatch({ type: "cancelled", requestId });
        setNotice("Generation cancelled.");
      } else {
        dispatch({ type: "failed", requestId, error: normalized });
      }
    }
  }

  async function cancelGeneration() {
    if (generation.status !== "generating") return;
    setNotice("Stopping generation…");
    try {
      const result = await eidosApi.cancelGeneration(generation.requestId);
      pendingCancellationId.current = result.cancelled
        ? null
        : generation.requestId;
    } catch (error) {
      setInputError(normalizeError(error));
    }
  }

  async function saveResult() {
    if (generation.status !== "ready") return;
    dispatch({ type: "saving" });
    try {
      const saved = await eidosApi.saveOutput(generation.result.attemptId);
      if (saved) setNotice(`Saved to ${saved.path}`);
      dispatch({ type: "saveFinished" });
    } catch (error) {
      dispatch({ type: "saveFailed", error: normalizeError(error) });
    }
  }

  function startNewGeneration() {
    dispatch({ type: "reset" });
    setPrompt("");
    setInputError(null);
    setNotice(null);
    window.setTimeout(() => promptRef.current?.focus(), 0);
  }

  function handleHistoryAttemptDeleted(attemptId: string) {
    if (
      generation.status === "ready" &&
      generation.result.attemptId === attemptId
    ) {
      dispatch({ type: "reset" });
      setNotice(null);
    }
  }

  async function loadHistoryAttempt(attempt: HistoryAttempt) {
    if (generation.status === "generating") {
      throw new Error("Wait for the current generation to finish before editing history.");
    }

    const nextReferences = attempt.references.length > 0
      ? await eidosApi.restoreHistoryReferences(attempt.id)
      : [];
    const previousReferences = references;

    const restoredModel = models.find(
      (model) => model.id === attempt.modelId && model.available,
    );
    const targetModel =
      restoredModel ??
      models.find((model) => model.isDefault && model.available) ??
      models.find((model) => model.available) ??
      selectedModel;
    const nextAspectRatio =
      attempt.settings.aspectRatio &&
        targetModel.supportedAspectRatios.includes(attempt.settings.aspectRatio)
        ? attempt.settings.aspectRatio
        : "auto";
    const nextResolution =
      attempt.settings.resolution &&
        targetModel.supportedResolutions.includes(attempt.settings.resolution)
        ? attempt.settings.resolution
        : "auto";

    setPrompt(attempt.prompt);
    setSelectedModelId(targetModel.id);
    setSettingsByModel((current) => ({
      ...current,
      [targetModel.id]: {
        aspectRatio: nextAspectRatio,
        resolution: nextResolution,
      },
    }));
    setReferences(nextReferences);
    setInputError(null);
    setNotice(
      restoredModel
        ? null
        : "That saved model is no longer available. Eidos selected the default model instead.",
    );
    dispatch({ type: "reset" });

    try {
      await Promise.all(
        previousReferences.map(({ token }) => eidosApi.discardReference(token)),
      );
    } catch (error) {
      setInputError(normalizeError(error));
    }
    window.setTimeout(() => promptRef.current?.focus(), 0);
  }

  const busy = generation.status === "generating";
  const generationError =
    generation.status === "error" ? generation.error : inputError;

  return {
    prompt,
    setPrompt,
    promptRef,
    references,
    models,
    selectedModel,
    selectedModelId: selectedModel.id,
    selectModel,
    maxReferences,
    maxReferenceTotalBytes,
    referenceTotalBytes,
    referenceBusy,
    aspectRatio,
    setAspectRatio: (value: string) => setModelSetting("aspectRatio", value),
    resolution,
    setResolution: (value: string) => setModelSetting("resolution", value),
    referenceLimitExceeded,
    generation,
    generationError,
    notice,
    busy,
    chooseReferences,
    removeReference,
    moveReference,
    shiftReference,
    generate,
    cancelGeneration,
    saveResult,
    startNewGeneration,
    handleHistoryAttemptDeleted,
    loadHistoryAttempt,
    aspectRatioOptions: ["auto", ...selectedModel.supportedAspectRatios],
    resolutionOptions: ["auto", ...selectedModel.supportedResolutions],
  };
}

export type StudioController = ReturnType<typeof useStudioController>;
