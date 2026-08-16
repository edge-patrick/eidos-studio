import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import type {
  AppError,
  AppStatus,
  GenerateRequest,
  GenerationJobEvent,
  HistoryCursor,
  HistoryPage,
  JobAccepted,
  ReferenceSelection,
  SaveResult,
} from "./types";

const GENERATION_JOB_EVENT = "generation-job-updated";
const HISTORY_THUMBNAILS_UPDATED_EVENT = "history-thumbnails-updated";

export function normalizeError(error: unknown): AppError {
  if (typeof error === "object" && error !== null && "message" in error) {
    const candidate = error as Partial<AppError>;
    return {
      kind: candidate.kind ?? "internal",
      message: String(candidate.message),
      retryable: candidate.retryable ?? true,
      details: candidate.details,
    };
  }

  if (typeof error === "string") {
    try {
      return normalizeError(JSON.parse(error));
    } catch {
      return {
        kind: "internal",
        message: error,
        retryable: true,
      };
    }
  }

  return {
    kind: "internal",
    message: "Eidos encountered an unexpected problem.",
    retryable: true,
  };
}

export const eidosApi = {
  getStatus: () => invoke<AppStatus>("get_app_status"),
  saveApiKey: (apiKey: string) =>
    invoke<AppStatus>("save_api_key", { apiKey }),
  removeApiKey: () => invoke<void>("remove_api_key"),
  selectReference: () =>
    invoke<ReferenceSelection | null>("select_reference_image"),
  discardReference: (token: string) =>
    invoke<void>("discard_reference", { token }),
  startGeneration: (request: GenerateRequest) =>
    invoke<JobAccepted>("start_generation", { request }),
  cancelGeneration: (requestId: string) =>
    invoke<{ cancelled: boolean }>("cancel_generation", { requestId }),
  saveOutput: (attemptId: string) =>
    invoke<SaveResult | null>("save_output", { attemptId }),
  listHistory: (cursor: HistoryCursor | null = null, limit = 60) =>
    invoke<HistoryPage>("list_history", { cursor, limit }),
  restoreHistoryReference: (attemptId: string) =>
    invoke<ReferenceSelection | null>("restore_history_reference", {
      attemptId,
    }),
  deleteHistoryAttempt: (attemptId: string) =>
    invoke<{ deleted: boolean }>("delete_history_attempt", { attemptId }),
  listenToGenerationJobs: (
    handler: (event: GenerationJobEvent) => void,
  ): Promise<UnlistenFn> =>
    listen<GenerationJobEvent>(GENERATION_JOB_EVENT, ({ payload }) =>
      handler(payload),
    ),
  listenToHistoryThumbnails: (handler: () => void): Promise<UnlistenFn> =>
    listen(HISTORY_THUMBNAILS_UPDATED_EVENT, handler),
  assetUrl: (path: string) => convertFileSrc(path),
};
