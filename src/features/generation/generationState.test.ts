import { describe, expect, it } from "vitest";
import {
  generationReducer,
  initialGenerationState,
} from "./generationState";

const result = {
  attemptId: "request-1",
  assetPath: "/managed/output.png",
  mimeType: "image/png",
  width: 1024,
  height: 1024,
  durationMs: 1000,
  modelId: "test/model",
};

describe("generationReducer", () => {
  it("ignores terminal events from another concurrent job", () => {
    const generating = generationReducer(initialGenerationState, {
      type: "started",
      requestId: "request-1",
    });

    const unchanged = generationReducer(generating, {
      type: "succeeded",
      requestId: "request-2",
      result,
    });

    expect(unchanged).toEqual(generating);
  });

  it("preserves a result when saving fails", () => {
    const generating = generationReducer(initialGenerationState, {
      type: "started",
      requestId: "request-1",
    });
    const ready = generationReducer(generating, {
      type: "succeeded",
      requestId: "request-1",
      result,
    });
    const failedSave = generationReducer(ready, {
      type: "saveFailed",
      error: {
        kind: "file",
        message: "Could not save",
        retryable: true,
      },
    });

    expect(failedSave.status).toBe("ready");
    if (failedSave.status === "ready") {
      expect(failedSave.result).toEqual(result);
      expect(failedSave.actionError?.kind).toBe("file");
    }
  });
});
