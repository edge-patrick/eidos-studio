import type { AppError, GenerationResult } from "../../shared/types";

export type GenerationState =
  | { status: "idle" }
  | { status: "generating"; requestId: string }
  | {
      status: "ready";
      result: GenerationResult;
      saving: boolean;
      actionError?: AppError;
    }
  | { status: "error"; error: AppError };

export type GenerationAction =
  | { type: "started"; requestId: string }
  | { type: "succeeded"; requestId: string; result: GenerationResult }
  | { type: "failed"; requestId: string; error: AppError }
  | { type: "cancelled"; requestId: string }
  | { type: "saving" }
  | { type: "saveFinished" }
  | { type: "saveFailed"; error: AppError }
  | { type: "reset" };

export const initialGenerationState: GenerationState = { status: "idle" };

export function generationReducer(
  state: GenerationState,
  action: GenerationAction,
): GenerationState {
  switch (action.type) {
    case "started":
      return { status: "generating", requestId: action.requestId };
    case "succeeded":
      return state.status === "generating" &&
        state.requestId === action.requestId
        ? { status: "ready", result: action.result, saving: false }
        : state;
    case "failed":
      return state.status === "generating" &&
        state.requestId === action.requestId
        ? { status: "error", error: action.error }
        : state;
    case "cancelled":
      return state.status === "generating" &&
        state.requestId === action.requestId
        ? { status: "idle" }
        : state;
    case "saving":
      return state.status === "ready"
        ? { ...state, saving: true, actionError: undefined }
        : state;
    case "saveFinished":
      return state.status === "ready" ? { ...state, saving: false } : state;
    case "saveFailed":
      return state.status === "ready"
        ? { ...state, saving: false, actionError: action.error }
        : state;
    case "reset":
      return { status: "idle" };
  }
}
