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
import type { GenerationJobEvent } from "./shared/types";

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
let generationListener: GenerationListener | undefined;

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
    generationListener = undefined;
    listenMock.mockImplementation(async (_event, handler) => {
      generationListener = handler as unknown as GenerationListener;
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

    await waitFor(() => expect(generationListener).toBeDefined());
    const request = invokeMock.mock.calls[1][1] as {
      request: { requestId: string };
    };
    act(() => {
      generationListener?.({
        payload: {
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
        },
      });
    });

    expect(await screen.findByAltText("Generated result")).toHaveAttribute(
      "src",
      "asset://localhost//managed/output.png",
    );
    expect(convertFileSrcMock).toHaveBeenCalledWith("/managed/output.png");
  });
});
