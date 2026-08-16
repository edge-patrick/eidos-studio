import { useEffect, useReducer, useRef, useState } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { eidosApi, normalizeError } from "../../shared/eidosApi";
import type {
  AppError,
  AppStatus,
  GenerateRequest,
  HistoryAttempt,
  ReferenceSelection,
} from "../../shared/types";
import {
  generationReducer,
  initialGenerationState,
} from "./generationState";

export function useStudioController(status: AppStatus) {
  const [prompt, setPrompt] = useState("");
  const [reference, setReference] = useState<ReferenceSelection | null>(null);
  const [referenceBusy, setReferenceBusy] = useState(false);
  const [aspectRatio, setAspectRatio] = useState("auto");
  const [resolution, setResolution] = useState("auto");
  const [inputError, setInputError] = useState<AppError | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [generation, dispatch] = useReducer(
    generationReducer,
    initialGenerationState,
  );
  const activeRequestId = useRef<string | null>(null);
  const eventListenerReady = useRef<Promise<UnlistenFn> | null>(null);
  const promptRef = useRef<HTMLTextAreaElement>(null);

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

  async function chooseReference() {
    setReferenceBusy(true);
    setInputError(null);
    try {
      const selection = await eidosApi.selectReference();
      if (!selection) return;
      const previous = reference;
      setReference(selection);
      if (previous) await eidosApi.discardReference(previous.token);
    } catch (error) {
      setInputError(normalizeError(error));
    } finally {
      setReferenceBusy(false);
    }
  }

  async function removeReference() {
    if (!reference) return;
    const token = reference.token;
    setReference(null);
    try {
      await eidosApi.discardReference(token);
    } catch (error) {
      setInputError(normalizeError(error));
    }
  }

  async function generate() {
    if (
      !status.hasApiKey ||
      !prompt.trim() ||
      generation.status === "generating"
    ) {
      return;
    }
    const requestId = crypto.randomUUID();
    const request: GenerateRequest = {
      requestId,
      prompt,
      referenceToken: reference?.token ?? null,
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
    } catch (error) {
      activeRequestId.current = null;
      dispatch({
        type: "failed",
        requestId,
        error: normalizeError(error),
      });
    }
  }

  async function cancelGeneration() {
    if (generation.status !== "generating") return;
    setNotice("Stopping generation…");
    try {
      await eidosApi.cancelGeneration(generation.requestId);
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

    const nextReference = attempt.reference
      ? await eidosApi.restoreHistoryReference(attempt.id)
      : null;
    const previousReference = reference;

    setPrompt(attempt.prompt);
    setAspectRatio(
      attempt.settings.aspectRatio &&
        status.supportedAspectRatios.includes(attempt.settings.aspectRatio)
        ? attempt.settings.aspectRatio
        : "auto",
    );
    setResolution(
      attempt.settings.resolution &&
        status.supportedResolutions.includes(attempt.settings.resolution)
        ? attempt.settings.resolution
        : "auto",
    );
    setReference(nextReference);
    setInputError(null);
    setNotice(null);
    dispatch({ type: "reset" });

    if (previousReference) {
      try {
        await eidosApi.discardReference(previousReference.token);
      } catch (error) {
        setInputError(normalizeError(error));
      }
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
    reference,
    referenceBusy,
    aspectRatio,
    setAspectRatio,
    resolution,
    setResolution,
    generation,
    generationError,
    notice,
    busy,
    chooseReference,
    removeReference,
    generate,
    cancelGeneration,
    saveResult,
    startNewGeneration,
    handleHistoryAttemptDeleted,
    loadHistoryAttempt,
    aspectRatioOptions: ["auto", ...status.supportedAspectRatios],
    resolutionOptions: ["auto", ...status.supportedResolutions],
  };
}

export type StudioController = ReturnType<typeof useStudioController>;
