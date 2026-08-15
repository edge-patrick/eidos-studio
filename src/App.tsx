import { FormEvent, useEffect, useState } from "react";
import "./App.css";
import { LoadingScreen } from "./components/LoadingScreen";
import { WorkspaceScreen } from "./features/generation/WorkspaceScreen";
import { useStudioController } from "./features/generation/useStudioController";
import { OnboardingScreen } from "./features/onboarding/OnboardingScreen";
import { eidosApi, normalizeError } from "./shared/eidosApi";
import type { AppError, AppStatus } from "./shared/types";

type Screen = "loading" | "onboarding" | "workspace";

const fallbackStatus: AppStatus = {
  hasApiKey: false,
  modelId: "",
  modelName: "",
  supportedAspectRatios: [],
  supportedResolutions: [],
};

function App() {
  const [screen, setScreen] = useState<Screen>("loading");
  const [status, setStatus] = useState<AppStatus>(fallbackStatus);
  const [apiKey, setApiKey] = useState("");
  const [connecting, setConnecting] = useState(false);
  const [changingKey, setChangingKey] = useState(false);
  const [onboardingError, setOnboardingError] = useState<AppError | null>(null);
  const studio = useStudioController(status);

  useEffect(() => {
    eidosApi
      .getStatus()
      .then((nextStatus) => {
        setStatus(nextStatus);
        setScreen(nextStatus.hasApiKey ? "workspace" : "onboarding");
      })
      .catch((error) => {
        setOnboardingError(normalizeError(error));
        setScreen("onboarding");
      });
  }, []);

  async function connectKey(event: FormEvent) {
    event.preventDefault();
    if (!apiKey.trim()) return;
    setConnecting(true);
    setOnboardingError(null);
    try {
      const nextStatus = await eidosApi.saveApiKey(apiKey);
      setApiKey("");
      setStatus(nextStatus);
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
      setStatus((current) => ({ ...current, hasApiKey: false }));
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
        setApiKey={setApiKey}
        connecting={connecting}
        changingKey={changingKey}
        hasApiKey={status.hasApiKey}
        error={onboardingError}
        onSubmit={(event) => void connectKey(event)}
        onBack={() => setScreen("workspace")}
        onForget={() => void forgetKey()}
      />
    );
  }

  return (
    <WorkspaceScreen
      status={status}
      studio={studio}
      onChangeKey={() => {
        setChangingKey(true);
        setOnboardingError(null);
        setScreen("onboarding");
      }}
    />
  );
}

export default App;
