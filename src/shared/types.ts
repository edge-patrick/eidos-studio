export interface AppStatus {
  hasApiKey: boolean;
  modelId: string;
  modelName: string;
  supportedAspectRatios: string[];
  supportedResolutions: string[];
}

export interface AppError {
  kind: string;
  message: string;
  retryable: boolean;
  details?: string;
}

export interface ReferenceSelection {
  token: string;
  fileName: string;
  mimeType: string;
  width: number;
  height: number;
  assetPath: string;
}

export interface GenerateRequest {
  requestId: string;
  prompt: string;
  referenceToken: string | null;
  aspectRatio: string | null;
  resolution: string | null;
}

export interface GenerationResult {
  attemptId: string;
  assetPath: string;
  mimeType: string;
  width: number;
  height: number;
  costUsd?: number;
  durationMs: number;
  modelId: string;
  providerName?: string;
}

export type GenerationJobStatus = "succeeded" | "failed" | "cancelled";

export interface GenerationJobEvent {
  requestId: string;
  status: GenerationJobStatus;
  result?: GenerationResult;
  error?: AppError;
}

export interface JobAccepted {
  requestId: string;
}

export interface SaveResult {
  path: string;
}
