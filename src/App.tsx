import { FormEvent, useEffect, useState } from "react";
import "./App.css";
import { LoadingScreen } from "./components/LoadingScreen";
import { StudioHeader, type StudioView } from "./components/StudioHeader";
import { WorkspaceScreen } from "./features/generation/WorkspaceScreen";
import { useStudioController } from "./features/generation/useStudioController";
import { LibraryScreen } from "./features/history/LibraryScreen";
import { OnboardingScreen } from "./features/onboarding/OnboardingScreen";
import { eidosApi, normalizeError } from "./shared/eidosApi";
import type { AppError, AppStatus, ImageModel } from "./shared/types";

type Screen = "loading" | "onboarding" | "workspace";

const fallbackStatus: AppStatus = {
  hasApiKey: false,
  apiKeyPreview: null,
  modelId: "",
  modelName: "",
  supportedAspectRatios: [],
  supportedResolutions: [],
  maxReferences: 14,
  maxReferenceTotalBytes: 48 * 1024 * 1024,
};

function App() {
  const [screen, setScreen] = useState<Screen>("loading");
  const [status, setStatus] = useState<AppStatus>(fallbackStatus);
  const [models, setModels] = useState<ImageModel[]>([]);
  const [apiKey, setApiKey] = useState("");
  const [connecting, setConnecting] = useState(false);
  const [changingKey, setChangingKey] = useState(false);
  const [studioView, setStudioView] = useState<StudioView>("create");
  const [onboardingError, setOnboardingError] = useState<AppError | null>(null);
  const studio = useStudioController(status, models);

  useEffect(() => {
    let active = true;
    const statusRequest = eidosApi.getStatus();
    const modelRequest = eidosApi.listImageModels();

    void modelRequest
      .then((nextModels) => {
        if (active && nextModels.length > 0) setModels(nextModels);
      })
      .catch(() => undefined);

    void statusRequest
      .then((nextStatus) => {
        if (!active) return;
        setStatus(nextStatus);
        setModels((current) =>
          current.length > 0 ? current : [modelFromStatus(nextStatus)],
        );
        setScreen(nextStatus.hasApiKey ? "workspace" : "onboarding");
      })
      .catch((error) => {
        if (!active) return;
        setOnboardingError(normalizeError(error));
        setScreen("onboarding");
      });

    return () => {
      active = false;
    };
  }, []);

  async function connectKey(event: FormEvent) {
    event.preventDefault();
    if (!apiKey.trim()) return;
    setConnecting(true);
    setOnboardingError(null);
    try {
      const nextStatus = await eidosApi.saveApiKey(apiKey);
      const nextModels = await eidosApi.listImageModels();
      setApiKey("");
      setStatus(nextStatus);
      setModels(nextModels.length > 0 ? nextModels : [modelFromStatus(nextStatus)]);
      setChangingKey(false);
      setScreen("workspace");
    } catch (error) {
      setOnboardingError(normalizeError(error));
    } finally {
      setConnecting(false);
    }
  }

  async function forgetKey() {
    setConnecting(true);
    setOnboardingError(null);
    try {
      await eidosApi.removeApiKey();
      setStatus((current) => ({
        ...current,
        hasApiKey: false,
        apiKeyPreview: null,
      }));
      setChangingKey(false);
      setApiKey("");
      setScreen("onboarding");
    } catch (error) {
      setOnboardingError(normalizeError(error));
    } finally {
      setConnecting(false);
    }
  }

  if (screen === "loading") return <LoadingScreen />;

  if (screen === "onboarding") {
    return (
      <OnboardingScreen
        apiKey={apiKey}
        apiKeyPreview={status.apiKeyPreview}
        setApiKey={setApiKey}
        connecting={connecting}
        changingKey={changingKey}
        hasApiKey={status.hasApiKey}
        error={onboardingError}
        onSubmit={(event) => void connectKey(event)}
        onBack={() => {
          setChangingKey(false);
          setScreen("workspace");
        }}
        onChangeKey={() => {
          setApiKey("");
          setOnboardingError(null);
          setChangingKey(true);
        }}
        onCancelChange={() => {
          setApiKey("");
          setOnboardingError(null);
          setChangingKey(false);
        }}
        onForget={() => void forgetKey()}
        onOpenLibrary={() => {
          setStudioView("library");
          setScreen("workspace");
        }}
      />
    );
  }

  return (
    <main className="studio-shell">
      <StudioHeader
        activeView={studioView}
        generating={studio.busy}
        connected={status.hasApiKey}
        onViewChange={setStudioView}
        onChangeKey={() => {
          setChangingKey(false);
          setOnboardingError(null);
          setScreen("onboarding");
        }}
      />
      <div className="studio-view-content" hidden={studioView !== "create"}>
        <WorkspaceScreen status={status} studio={studio} />
      </div>
      <div className="studio-view-content" hidden={studioView !== "library"}>
        <LibraryScreen
          active={studioView === "library"}
          status={status}
          models={models}
          generationBusy={studio.busy}
          onDeleted={studio.handleHistoryAttemptDeleted}
          onReuse={async (attempt) => {
            await studio.loadHistoryAttempt(attempt);
            setStudioView("create");
          }}
        />
      </div>
    </main>
  );
}

function modelFromStatus(status: AppStatus): ImageModel {
  return {
    id: status.modelId,
    name: status.modelName,
    provider: "Google",
    description: "Balanced image generation.",
    available: true,
    isDefault: true,
    supportedAspectRatios: status.supportedAspectRatios,
    supportedResolutions: status.supportedResolutions,
    supportedQualities: [],
    maxReferences: status.maxReferences,
    referenceConstraints: { maxBytes: 12 * 1024 * 1024 },
  };
}

export default App;
