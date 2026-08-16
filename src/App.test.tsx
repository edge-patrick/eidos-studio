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
import type { GenerationJobEvent, HistoryAttempt, HistoryCursor } from "./shared/types";

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
  attempts: HistoryAttempt[],
  nextCursor: HistoryCursor | null = null,
  totalCount = attempts.length,
) {
  return { attempts, nextCursor, totalCount };
}

function emitGeneration(payload: GenerationJobEvent) {
  generationListeners.forEach((listener) => listener({ payload }));
}

const appStatus = {
  hasApiKey: true,
  modelId: "google/gemini-3.1-flash-image",
  modelName: "Nano Banana",
  supportedAspectRatios: ["1:1", "2:3", "3:2", "16:9"],
  supportedResolutions: ["1K", "2K", "4K"],
};

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
    invokeMock.mockResolvedValueOnce({
      hasApiKey: false,
      modelId: "google/gemini-3.1-flash-image",
      modelName: "Nano Banana",
      supportedAspectRatios: ["1:1", "2:3", "3:2", "16:9"],
      supportedResolutions: ["1K", "2K", "4K"],
    });

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
    invokeMock.mockResolvedValueOnce(appStatus);

    render(<App />);

    expect(
      await screen.findByPlaceholderText("Describe the image you want to make…"),
    ).toBeInTheDocument();
    expect(screen.getByText("Your image appears here.")).toBeInTheDocument();
  });

  it("shows the active model and disabled upcoming models in the composer", async () => {
    invokeMock.mockResolvedValueOnce(appStatus);

    render(<App />);

    await screen.findByPlaceholderText("Describe the image you want to make…");
    fireEvent.click(screen.getByLabelText("Select image model"));

    expect(
      screen.getByRole("option", { name: /Nano Banana/ }),
    ).toHaveAttribute("aria-selected", "true");
    expect(
      screen.getByRole("option", { name: /GPT Image.*Coming soon/ }),
    ).toBeDisabled();
    expect(
      screen.getByRole("option", { name: /FLUX.*Coming soon/ }),
    ).toBeDisabled();
    expect(
      screen.getByRole("option", { name: /Ideogram.*Coming soon/ }),
    ).toBeDisabled();
  });

  it("dismisses the model picker outside and with Escape", async () => {
    invokeMock.mockResolvedValueOnce(appStatus);

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
      expect(invokeMock).toHaveBeenNthCalledWith(2, "start_generation", {
        request: {
          requestId: expect.any(String),
          prompt: "A wide night landscape",
          referenceToken: null,
          aspectRatio: "16:9",
          resolution: "2K",
        },
      });
    });
  });

  it("renders a completed job from its managed asset path", async () => {
    invokeMock
      .mockResolvedValueOnce(appStatus)
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
    const request = invokeMock.mock.calls[1][1] as {
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
  });

  it("opens successful, failed, and cancelled history and edits a failed prompt", async () => {
    invokeMock.mockImplementation(async (command) => {
      if (command === "get_app_status") return appStatus;
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
            modelId: appStatus.modelId,
            status: "failed",
            settings: { aspectRatio: "16:9", resolution: "1K" },
            createdAt: "2026-08-16T12:30:00Z",
            durationMs: 500,
            errorKind: "provider",
            errorMessage: "The provider rejected this request.",
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
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));

    expect(
      await screen.findByDisplayValue("A prompt worth revising"),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "16:9" })).toHaveClass("selected");
    expect(screen.getByRole("button", { name: "1K" })).toHaveClass("selected");
  });

  it("loads older history in bounded pages", async () => {
    let historyCalls = 0;
    invokeMock.mockImplementation(async (command) => {
      if (command === "get_app_status") return appStatus;
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
