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

export interface GenerationSettings {
  aspectRatio?: string;
  resolution?: string;
}

export interface HistoryAsset {
  assetPath: string;
  thumbnailPath?: string;
  mimeType: string;
  width: number;
  height: number;
}

export interface HistoryAttempt {
  id: string;
  prompt: string;
  modelId: string;
  status: "succeeded" | "failed" | "cancelled";
  settings: GenerationSettings;
  createdAt: string;
  completedAt?: string;
  durationMs?: number;
  costUsd?: number;
  providerName?: string;
  errorKind?: string;
  errorMessage?: string;
  output?: HistoryAsset;
  reference?: HistoryAsset;
}

export interface HistoryCursor {
  createdAt: string;
  id: string;
}

export interface HistoryPage {
  attempts: HistoryAttempt[];
  nextCursor?: HistoryCursor;
  totalCount: number;
}
