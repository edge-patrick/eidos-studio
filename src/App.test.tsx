import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import type {
  GenerationJobEvent,
  HistoryAttempt,
  HistoryCursor,
  ImageModel,
} from "./shared/types";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
  convertFileSrc: vi.fn((path: string) => `asset://localhost/${path}`),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);
const listenMock = vi.mocked(listen);
const convertFileSrcMock = vi.mocked(convertFileSrc);
type GenerationListener = (event: { payload: GenerationJobEvent }) => void;
let generationListeners: GenerationListener[] = [];

function historyPage(
  attempts: Array<Omit<HistoryAttempt, "references"> & { references?: HistoryAttempt["references"] }>,
  nextCursor: HistoryCursor | null = null,
  totalCount = attempts.length,
) {
  return {
    attempts: attempts.map((attempt) => ({ ...attempt, references: attempt.references ?? [] })),
    nextCursor,
    totalCount,
  };
}

function emitGeneration(payload: GenerationJobEvent) {
  generationListeners.forEach((listener) => listener({ payload }));
}

const appStatus = {
  hasApiKey: true,
  modelId: "google/gemini-3.1-flash-image",
  modelName: "Nano Banana 2",
  supportedAspectRatios: ["1:1", "2:3", "3:2", "16:9"],
  supportedResolutions: ["1K", "2K", "4K"],
  maxReferences: 14,
  maxReferenceTotalBytes: 48 * 1024 * 1024,
};

const imageModels: ImageModel[] = [
  {
    id: "google/gemini-3.1-flash-lite-image",
    name: "Nano Banana 2 Lite",
    provider: "Google",
    description: "Fast, efficient generation at 1K resolution.",
    available: true,
    isDefault: false,
    supportedAspectRatios: ["1:1", "2:3", "3:2", "16:9"],
    supportedResolutions: ["1K"],
    maxReferences: 14,
  },
  {
    id: appStatus.modelId,
    name: appStatus.modelName,
    provider: "Google",
    description: "Balanced speed and quality from 1K through 4K.",
    available: true,
    isDefault: true,
    supportedAspectRatios: appStatus.supportedAspectRatios,
    supportedResolutions: appStatus.supportedResolutions,
    maxReferences: 14,
  },
  {
    id: "google/gemini-3-pro-image",
    name: "Nano Banana Pro",
    provider: "Google",
    description: "Higher-quality generation for detail-sensitive work.",
    available: true,
    isDefault: false,
    supportedAspectRatios: ["1:1", "2:3", "3:2", "16:9"],
    supportedResolutions: ["1K", "2K", "4K"],
    maxReferences: 14,
  },
  {
    id: "openai/gpt-image-2",
    name: "GPT Image 2",
    provider: "OpenAI",
    description:
      "Fast, high-quality generation and editing with high-fidelity references.",
    available: true,
    isDefault: false,
    supportedAspectRatios: ["1:1", "2:3", "3:2", "16:9"],
    supportedResolutions: [],
    maxReferences: 14,
  },
];

describe("App", () => {
  beforeEach(() => {
    cleanup();
    invokeMock.mockReset();
    listenMock.mockReset();
    generationListeners = [];
    listenMock.mockImplementation(async (event, handler) => {
      if (event === "generation-job-updated") {
        generationListeners.push(handler as unknown as GenerationListener);
      }
      return () => undefined;
    });
    convertFileSrcMock.mockClear();
  });

  it("shows onboarding when no API key is stored", async () => {
    invokeMock
      .mockResolvedValueOnce({ ...appStatus, hasApiKey: false })
      .mockResolvedValueOnce(imageModels);

    render(<App />);

    expect(
      await screen.findByRole("heading", { name: "Bring your own key" }),
    ).toBeInTheDocument();
    expect(screen.queryByText("Local image workspace")).not.toBeInTheDocument();
    expect(screen.queryByText("01 / CONNECT")).not.toBeInTheDocument();
    expect(screen.queryByText("EI")).not.toBeInTheDocument();
    expect(screen.queryByText("01")).not.toBeInTheDocument();
    expect(
      screen.getByText(/sent to OpenRouter and the model provider/),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Open local library" }),
    ).toBeInTheDocument();
  });

  it("opens the local library without an API key", async () => {
    invokeMock.mockImplementation(async (command) => {
      if (command === "get_app_status") {
        return { ...appStatus, hasApiKey: false };
      }
      if (command === "list_image_models") return imageModels;
      if (command === "list_history") return historyPage([]);
      throw new Error(`Unexpected command: ${command}`);
    });

    render(<App />);

    fireEvent.click(
      await screen.findByRole("button", { name: "Open local library" }),
    );

    expect(
      await screen.findByRole("heading", { name: "Library" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /OpenRouter not connected/ }),
    ).toHaveClass("disconnected");

    fireEvent.click(screen.getByRole("button", { name: "Create" }));
    const prompt = screen.getByPlaceholderText(
      "Describe the image you want to make…",
    );
    fireEvent.change(prompt, { target: { value: "Should not be recorded" } });
    expect(screen.getByRole("button", { name: "Generate image" })).toBeDisabled();
    fireEvent.keyDown(prompt, { key: "Enter", metaKey: true });
    expect(invokeMock.mock.calls.some(([command]) => command === "start_generation"))
      .toBe(false);
  });

  it("opens the workspace when a key is already stored", async () => {
    invokeMock
      .mockResolvedValueOnce(appStatus)
      .mockResolvedValueOnce(imageModels);

    render(<App />);

    expect(
      await screen.findByPlaceholderText("Describe the image you want to make…"),
    ).toBeInTheDocument();
    expect(screen.getByText("Your image appears here.")).toBeInTheDocument();
  });

  it("opens the workspace without waiting for model discovery", async () => {
    invokeMock.mockImplementation(async (command) => {
      if (command === "get_app_status") return appStatus;
      if (command === "list_image_models") {
        return new Promise(() => undefined);
      }
      throw new Error(`Unexpected command: ${command}`);
    });

    render(<App />);

    expect(
      await screen.findByPlaceholderText("Describe the image you want to make…"),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("Select image model")).toHaveTextContent(
      "google/gemini-3.1-flash-image",
    );
  });

  it("keeps the catalog default when model discovery resolves before status", async () => {
    let releaseStatus!: () => void;
    const delayedStatus = new Promise<typeof appStatus>((resolve) => {
      releaseStatus = () => resolve({ ...appStatus, hasApiKey: false });
    });
    invokeMock.mockImplementation(async (command) => {
      if (command === "get_app_status") return delayedStatus;
      if (command === "list_image_models") return imageModels;
      if (command === "save_api_key") return appStatus;
      throw new Error(`Unexpected command: ${command}`);
    });

    render(<App />);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 20));
    });
    await act(async () => {
      releaseStatus();
      await new Promise((resolve) => setTimeout(resolve, 20));
    });

    fireEvent.change(
      await screen.findByLabelText("OpenRouter API key"),
      { target: { value: "sk-or-v1-test-key-1234567890" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Connect OpenRouter" }));

    await screen.findByPlaceholderText("Describe the image you want to make…");
    expect(screen.getByLabelText("Select image model")).toHaveTextContent(
      "google/gemini-3.1-flash-image",
    );
  });

  it("selects models across providers and uses their picker icons", async () => {
    invokeMock
      .mockResolvedValueOnce(appStatus)
      .mockResolvedValueOnce(imageModels);

    render(<App />);

    await screen.findByPlaceholderText("Describe the image you want to make…");
    fireEvent.click(screen.getByLabelText("Select image model"));

    expect(
      screen.getByRole("option", { name: /^Nano Banana 2 Google/ }),
    ).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("option", { name: /Nano Banana 2 Lite/ })).toBeEnabled();
    expect(screen.getByRole("option", { name: /Nano Banana Pro/ })).toBeEnabled();
    const gptImageOption = screen.getByRole("option", {
      name: /GPT Image 2 OpenAI/,
    });
    expect(gptImageOption).toBeEnabled();
    expect(gptImageOption.querySelector("img")).toHaveAttribute(
      "src",
      expect.stringContaining("gpt-image.png"),
    );

    fireEvent.click(screen.getByRole("option", { name: /Nano Banana 2 Lite/ }));
    expect(screen.getByLabelText("Select image model")).toHaveTextContent(
      "Nano Banana 2 Lite",
    );
    expect(screen.queryByRole("button", { name: "2K" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "1K" })).toBeInTheDocument();

    fireEvent.click(screen.getByLabelText("Select image model"));
    fireEvent.click(screen.getByRole("option", { name: /GPT Image 2 OpenAI/ }));
    expect(screen.getByLabelText("Select image model")).toHaveTextContent(
      "GPT Image 2",
    );
    expect(
      screen.getByText("GPT Image 2 uses the provider's default output size."),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("group", { name: "Resolution" }),
    ).not.toBeInTheDocument();
  });

  it("shows catalog models that are temporarily unavailable", async () => {
    const unavailableModels = imageModels.map((model, index) =>
      index === 0
        ? {
            ...model,
            available: false,
            unavailableReason: "Unavailable on OpenRouter. Try again later.",
            supportedAspectRatios: [],
            supportedResolutions: [],
            maxReferences: 0,
          }
        : model,
    );
    invokeMock
      .mockResolvedValueOnce(appStatus)
      .mockResolvedValueOnce(unavailableModels);

    render(<App />);

    await screen.findByPlaceholderText("Describe the image you want to make…");
    fireEvent.click(screen.getByLabelText("Select image model"));

    const unavailable = screen.getByRole("option", {
      name: /Nano Banana 2 Lite.*Unavailable on OpenRouter. Try again later./,
    });
    expect(unavailable).toBeDisabled();
    expect(unavailable).toHaveTextContent(
      "Unavailable on OpenRouter. Try again later.",
    );
  });

  it("explains when the selected model does not support references", async () => {
    const modelsWithoutReferences = imageModels.map((model) =>
      model.id === appStatus.modelId ? { ...model, maxReferences: 0 } : model,
    );
    invokeMock
      .mockResolvedValueOnce(appStatus)
      .mockResolvedValueOnce(modelsWithoutReferences);

    render(<App />);

    await screen.findByPlaceholderText("Describe the image you want to make…");
    expect(screen.getByText("Not supported")).toBeInTheDocument();
    expect(
      screen.getByText("Nano Banana 2 does not support reference images."),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /Add reference images/ }),
    ).not.toBeInTheDocument();
  });

  it("dismisses the model picker outside and with Escape", async () => {
    invokeMock
      .mockResolvedValueOnce(appStatus)
      .mockResolvedValueOnce(imageModels);

    render(<App />);

    const prompt = await screen.findByPlaceholderText(
      "Describe the image you want to make…",
    );
    const pickerButton = screen.getByLabelText("Select image model");
    const picker = pickerButton.closest("details");

    fireEvent.click(pickerButton);
    expect(picker).toHaveAttribute("open");

    fireEvent.pointerDown(prompt);
    expect(picker).not.toHaveAttribute("open");

    fireEvent.click(pickerButton);
    expect(picker).toHaveAttribute("open");

    fireEvent.keyDown(document, { key: "Escape" });
    expect(picker).not.toHaveAttribute("open");
    expect(pickerButton).toHaveFocus();
  });

  it("sends the selected numeric ratio and resolution", async () => {
    invokeMock
      .mockResolvedValueOnce(appStatus)
      .mockResolvedValueOnce(imageModels)
      .mockResolvedValueOnce({
        requestId: "attempt-1",
      });

    render(<App />);

    const prompt = await screen.findByPlaceholderText(
      "Describe the image you want to make…",
    );
    fireEvent.change(prompt, { target: { value: "A wide night landscape" } });
    fireEvent.click(screen.getByRole("button", { name: "16:9" }));
    fireEvent.click(screen.getByRole("button", { name: "2K" }));
    fireEvent.click(screen.getByRole("button", { name: "Generate image" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenNthCalledWith(3, "start_generation", {
        request: {
          requestId: expect.any(String),
          modelId: appStatus.modelId,
          prompt: "A wide night landscape",
          referenceTokens: [],
          aspectRatio: "16:9",
          resolution: "2K",
        },
      });
    });
  });

  it("selects, reorders, and submits multiple reference images", async () => {
    invokeMock.mockImplementation(async (command, args) => {
      if (command === "get_app_status") return appStatus;
      if (command === "list_image_models") return imageModels;
      if (command === "select_reference_image") {
        return [
          {
            token: "reference-a",
            fileName: "a.png",
            mimeType: "image/png",
            width: 800,
            height: 600,
            sizeBytes: 2 * 1024 * 1024,
            assetPath: "/managed/a.png",
            thumbnailPath: "/managed/a-thumbnail.png",
          },
          {
            token: "reference-b",
            fileName: "b.jpg",
            mimeType: "image/jpeg",
            width: 600,
            height: 800,
            sizeBytes: 3 * 1024 * 1024,
            assetPath: "/managed/b.jpg",
            thumbnailPath: "/managed/b-thumbnail.png",
          },
        ];
      }
      if (command === "start_generation") {
        return {
          requestId: (args as { request: { requestId: string } }).request.requestId,
        };
      }
      throw new Error(`Unexpected command: ${command}`);
    });

    render(<App />);
    const prompt = await screen.findByPlaceholderText(
      "Describe the image you want to make…",
    );
    fireEvent.click(
      screen.getByRole("button", { name: /Add reference images/ }),
    );

    expect(await screen.findByText("a.png")).toBeInTheDocument();
    expect(screen.getByText("b.jpg")).toBeInTheDocument();
    expect(convertFileSrcMock).toHaveBeenCalledWith("/managed/a-thumbnail.png");
    expect(convertFileSrcMock).toHaveBeenCalledWith("/managed/b-thumbnail.png");
    expect(screen.getByText("2 / 14")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Move b.jpg earlier" }));
    fireEvent.change(prompt, { target: { value: "Blend both references" } });
    fireEvent.click(screen.getByRole("button", { name: "Generate image" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("start_generation", {
        request: {
          requestId: expect.any(String),
          modelId: appStatus.modelId,
          prompt: "Blend both references",
          referenceTokens: ["reference-b", "reference-a"],
          aspectRatio: null,
          resolution: null,
        },
      });
    });
  });

  it("does not generate while reference images are still importing", async () => {
    let finishSelection!: (value: []) => void;
    invokeMock.mockImplementation(async (command) => {
      if (command === "get_app_status") return appStatus;
      if (command === "list_image_models") return imageModels;
      if (command === "select_reference_image") {
        return new Promise<[]>((resolve) => {
          finishSelection = resolve;
        });
      }
      if (command === "start_generation") {
        throw new Error("Generation started before reference import completed.");
      }
      throw new Error(`Unexpected command: ${command}`);
    });

    render(<App />);
    const prompt = await screen.findByPlaceholderText(
      "Describe the image you want to make…",
    );
    fireEvent.change(prompt, { target: { value: "Use the incoming references" } });
    fireEvent.click(
      screen.getByRole("button", { name: /Add reference images/ }),
    );

    const generate = screen.getByRole("button", { name: "Generate image" });
    expect(generate).toBeDisabled();
    fireEvent.keyDown(prompt, { key: "Enter", metaKey: true });
    expect(
      invokeMock.mock.calls.some(([command]) => command === "start_generation"),
    ).toBe(false);

    await act(async () => finishSelection([]));
    expect(generate).toBeEnabled();
  });

  it("retries cancellation when generation has not registered yet", async () => {
    let finishStart!: (value: { requestId: string }) => void;
    let requestId = "";
    let cancellationCalls = 0;
    invokeMock.mockImplementation(async (command, args) => {
      if (command === "get_app_status") return appStatus;
      if (command === "list_image_models") return imageModels;
      if (command === "start_generation") {
        requestId = (args as { request: { requestId: string } }).request.requestId;
        return new Promise<{ requestId: string }>((resolve) => {
          finishStart = resolve;
        });
      }
      if (command === "cancel_generation") {
        cancellationCalls += 1;
        return { cancelled: cancellationCalls > 1 };
      }
      throw new Error(`Unexpected command: ${command}`);
    });

    render(<App />);
    const prompt = await screen.findByPlaceholderText(
      "Describe the image you want to make…",
    );
    fireEvent.change(prompt, { target: { value: "Cancel while preparing" } });
    fireEvent.click(screen.getByRole("button", { name: "Generate image" }));
    fireEvent.click(
      await screen.findByRole("button", { name: "Cancel generation" }),
    );

    await waitFor(() => expect(cancellationCalls).toBe(1));
    await act(async () => finishStart({ requestId }));
    await waitFor(() => expect(cancellationCalls).toBe(2));
  });

  it("renders a completed job and clears the text for a new prompt", async () => {
    invokeMock
      .mockResolvedValueOnce(appStatus)
      .mockResolvedValueOnce(imageModels)
      .mockImplementationOnce(async (_command, args) => ({
        requestId: (args as { request: { requestId: string } }).request.requestId,
      }));

    render(<App />);
    const prompt = await screen.findByPlaceholderText(
      "Describe the image you want to make…",
    );
    fireEvent.change(prompt, { target: { value: "A quiet blue room" } });
    fireEvent.click(screen.getByRole("button", { name: "Generate image" }));

    await waitFor(() => expect(generationListeners.length).toBeGreaterThan(0));
    const request = invokeMock.mock.calls[2][1] as {
      request: { requestId: string };
    };
    act(() => {
      emitGeneration({
          requestId: request.request.requestId,
          status: "succeeded",
          result: {
            attemptId: request.request.requestId,
            assetPath: "/managed/output.png",
            mimeType: "image/png",
            width: 1024,
            height: 1024,
            durationMs: 1000,
            modelId: appStatus.modelId,
          },
      });
    });

    expect(await screen.findByAltText("Generated result")).toHaveAttribute(
      "src",
      "asset://localhost//managed/output.png",
    );
    expect(convertFileSrcMock).toHaveBeenCalledWith("/managed/output.png");

    fireEvent.click(screen.getByRole("button", { name: "New prompt" }));

    expect(prompt).toHaveValue("");
    expect(screen.queryByAltText("Generated result")).not.toBeInTheDocument();
    expect(screen.getByText("Your image appears here.")).toBeInTheDocument();
    await waitFor(() => expect(prompt).toHaveFocus());
  });

  it("opens successful, failed, and cancelled history and edits a failed prompt", async () => {
    invokeMock.mockImplementation(async (command) => {
      if (command === "get_app_status") return appStatus;
      if (command === "list_image_models") return imageModels;
      if (command === "list_history") {
        return historyPage([
          {
            id: "success-1",
            prompt: "A quiet violet observatory",
            modelId: appStatus.modelId,
            status: "succeeded",
            settings: { aspectRatio: "1:1", resolution: "2K" },
            createdAt: "2026-08-16T12:32:00Z",
            durationMs: 1200,
            costUsd: 0.04,
            output: {
              assetPath: "/managed/success.png",
              thumbnailPath: "/managed/thumbnails/success.png",
              mimeType: "image/png",
              width: 2048,
              height: 2048,
            },
          },
          {
            id: "failure-1",
            prompt: "A prompt worth revising",
            modelId: imageModels[0].id,
            status: "failed",
            settings: { aspectRatio: "16:9", resolution: "1K" },
            createdAt: "2026-08-16T12:30:00Z",
            durationMs: 500,
            errorKind: "provider",
            errorMessage: "The provider rejected this request.",
            references: [
              {
                assetPath: "/managed/reference-1.png",
                thumbnailPath: "/managed/thumbnails/reference-1.png",
                mimeType: "image/png",
                width: 640,
                height: 480,
              },
              {
                assetPath: "/managed/reference-2.jpg",
                thumbnailPath: "/managed/thumbnails/reference-2.png",
                mimeType: "image/jpeg",
                width: 480,
                height: 640,
              },
            ],
          },
          {
            id: "cancelled-1",
            prompt: "A generation stopped midway",
            modelId: appStatus.modelId,
            status: "cancelled",
            settings: { aspectRatio: "3:2", resolution: "2K" },
            createdAt: "2026-08-16T12:29:00Z",
            durationMs: 300,
            errorKind: "cancelled",
            errorMessage: "Generation cancelled by the user.",
          },
        ]);
      }
      if (command === "restore_history_reference") {
        return [
          {
            token: "saved-reference-1",
            fileName: "Saved reference 1",
            mimeType: "image/png",
            width: 640,
            height: 480,
            sizeBytes: 1024,
            assetPath: "/managed/saved-reference-1.png",
            thumbnailPath: "/managed/saved-reference-1-thumbnail.png",
          },
          {
            token: "saved-reference-2",
            fileName: "Saved reference 2",
            mimeType: "image/jpeg",
            width: 480,
            height: 640,
            sizeBytes: 2048,
            assetPath: "/managed/saved-reference-2.jpg",
            thumbnailPath: "/managed/saved-reference-2-thumbnail.png",
          },
        ];
      }
      throw new Error(`Unexpected command: ${command}`);
    });

    render(<App />);
    await screen.findByPlaceholderText("Describe the image you want to make…");
    fireEvent.click(screen.getByRole("button", { name: "Library" }));

    const successfulPrompt = await screen.findByText("A quiet violet observatory");
    const successfulCard = successfulPrompt.closest("button")!;
    expect(successfulCard.querySelectorAll(".history-card-preview img")).toHaveLength(2);
    expect(successfulCard.querySelector("img")).toHaveAttribute(
      "src",
      "asset://localhost//managed/thumbnails/success.png",
    );
    const failedPrompt = screen.getByText("A prompt worth revising");
    const cancelledPrompt = screen.getByText("A generation stopped midway");
    expect(screen.getByText("Generation failed")).toBeInTheDocument();
    expect(screen.getByText("Generation cancelled")).toBeInTheDocument();
    expect(screen.queryByRole("searchbox")).not.toBeInTheDocument();

    fireEvent.click(successfulCard);
    expect(screen.getByRole("button", { name: "Delete" })).toBeVisible();
    const inspectorImage = screen.getByRole("button", { name: "View image full size" });
    expect(inspectorImage.querySelectorAll("img")).toHaveLength(2);
    fireEvent.click(inspectorImage);
    expect(screen.getByRole("dialog", { name: "Full-size generated image" })).toBeInTheDocument();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("dialog", { name: "Full-size generated image" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Close details" })).toBeInTheDocument();

    fireEvent.click(cancelledPrompt.closest("button")!);
    expect(screen.getByText("Generation cancelled by the user.")).toBeInTheDocument();

    fireEvent.click(failedPrompt.closest("button")!);
    expect(screen.getByText("Image not made")).toBeInTheDocument();
    expect(screen.getByText("The provider rejected this request.")).toBeInTheDocument();
    expect(screen.getByAltText("Reference 1 used for generation")).toHaveAttribute(
      "src",
      "asset://localhost//managed/thumbnails/reference-1.png",
    );
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));

    expect(
      await screen.findByDisplayValue("A prompt worth revising"),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("Select image model")).toHaveTextContent(
      "Nano Banana 2 Lite",
    );
    expect(screen.getByRole("button", { name: "16:9" })).toHaveClass("selected");
    expect(screen.getByRole("button", { name: "1K" })).toHaveClass("selected");
    expect(await screen.findByText("Saved reference 1")).toBeInTheDocument();
    expect(screen.getByText("Saved reference 2")).toBeInTheDocument();
    expect(screen.getByText("2 / 14")).toBeInTheDocument();
  });

  it("loads older history in bounded pages", async () => {
    let historyCalls = 0;
    invokeMock.mockImplementation(async (command) => {
      if (command === "get_app_status") return appStatus;
      if (command === "list_image_models") return imageModels;
      if (command === "list_history") {
        historyCalls += 1;
        if (historyCalls === 1) {
          return historyPage(
            [{
              id: "newer",
              prompt: "Newest archived prompt",
              modelId: appStatus.modelId,
              status: "failed",
              settings: {},
              createdAt: "2026-08-16T12:32:00Z",
            }],
            { createdAt: "2026-08-16T12:32:00Z", id: "newer" },
            2,
          );
        }
        return historyPage([{
          id: "older",
          prompt: "Older archived prompt",
          modelId: appStatus.modelId,
          status: "cancelled",
          settings: {},
          createdAt: "2026-08-15T12:32:00Z",
        }], null, 2);
      }
      throw new Error(`Unexpected command: ${command}`);
    });

    render(<App />);
    await screen.findByPlaceholderText("Describe the image you want to make…");
    fireEvent.click(screen.getByRole("button", { name: "Library" }));

    expect(await screen.findByText("Newest archived prompt")).toBeInTheDocument();
    expect(screen.getByText("1 of 2 loaded")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Load more" }));

    expect(await screen.findByText("Older archived prompt")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Load more" })).not.toBeInTheDocument();
  });

  it("keeps loaded history visible when a background refresh fails", async () => {
    let historyCalls = 0;
    invokeMock.mockImplementation(async (command) => {
      if (command === "get_app_status") return appStatus;
      if (command === "list_image_models") return imageModels;
      if (command === "list_history") {
        historyCalls += 1;
        if (historyCalls > 1) throw new Error("database temporarily unavailable");
        return historyPage([{
          id: "cached-attempt",
          prompt: "Still visible from the cache",
          modelId: appStatus.modelId,
          status: "failed",
          settings: {},
          createdAt: "2026-08-16T12:32:00Z",
        }]);
      }
      throw new Error(`Unexpected command: ${command}`);
    });

    render(<App />);
    await screen.findByPlaceholderText("Describe the image you want to make…");
    fireEvent.click(screen.getByRole("button", { name: "Library" }));
    expect(await screen.findByText("Still visible from the cache")).toBeInTheDocument();
    await waitFor(() => expect(generationListeners.length).toBeGreaterThan(1));

    act(() => {
      emitGeneration({
        requestId: "unrelated-attempt",
        status: "failed",
        error: { kind: "provider", message: "Failed", retryable: true },
      });
    });

    expect(
      await screen.findByText(/Library refresh paused/),
    ).toBeInTheDocument();
    expect(screen.getByText("Still visible from the cache")).toBeInTheDocument();
    expect(screen.queryByText("History could not be opened.")).not.toBeInTheDocument();
  });

  it("resets Create when its displayed generation is deleted from Library", async () => {
    let attemptId = "";
    invokeMock.mockImplementation(async (command, args) => {
      if (command === "get_app_status") return appStatus;
      if (command === "list_image_models") return imageModels;
      if (command === "start_generation") {
        attemptId = (args as { request: { requestId: string } }).request.requestId;
        return { requestId: attemptId };
      }
      if (command === "list_history") {
        return historyPage([
          {
            id: attemptId,
            prompt: "A temporary result",
            modelId: appStatus.modelId,
            status: "succeeded",
            settings: {},
            createdAt: "2026-08-16T12:32:00Z",
            output: {
              assetPath: "/managed/temporary.png",
              mimeType: "image/png",
              width: 1024,
              height: 1024,
            },
          },
        ]);
      }
      if (command === "delete_history_attempt") return { deleted: true };
      throw new Error(`Unexpected command: ${command}`);
    });

    render(<App />);
    const prompt = await screen.findByPlaceholderText(
      "Describe the image you want to make…",
    );
    fireEvent.change(prompt, { target: { value: "A temporary result" } });
    fireEvent.click(screen.getByRole("button", { name: "Generate image" }));
    await waitFor(() => expect(attemptId).not.toBe(""));

    act(() => {
      emitGeneration({
          requestId: attemptId,
          status: "succeeded",
          result: {
            attemptId,
            assetPath: "/managed/temporary.png",
            mimeType: "image/png",
            width: 1024,
            height: 1024,
            durationMs: 1000,
            modelId: appStatus.modelId,
          },
      });
    });
    expect(await screen.findByAltText("Generated result")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Library" }));
    fireEvent.click(
      await screen.findByRole("button", { name: /A temporary result/ }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));
    fireEvent.click(
      screen
        .getAllByRole("button", { name: "Delete" })
        .find((button) => !button.hasAttribute("disabled"))!,
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("delete_history_attempt", {
        attemptId,
      });
    });
    fireEvent.click(screen.getByRole("button", { name: "Create" }));

    expect(screen.getByText("Your image appears here.")).toBeInTheDocument();
    expect(screen.queryByAltText("Generated result")).not.toBeInTheDocument();
  });
});
