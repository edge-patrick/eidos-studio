import { BrandMark } from "../../components/BrandMark";
import { ErrorMessage } from "../../components/ErrorMessage";
import {
  ArrowIcon,
  BackIcon,
  KeyIcon,
  LockIcon,
  Spinner,
  TrashIcon,
} from "../../components/Icons";
import type { AppError } from "../../shared/types";

interface OnboardingScreenProps {
  apiKey: string;
  setApiKey: (value: string) => void;
  connecting: boolean;
  changingKey: boolean;
  hasApiKey: boolean;
  error: AppError | null;
  onSubmit: (event: React.FormEvent) => void;
  onBack: () => void;
  onForget: () => void;
  onOpenLibrary: () => void;
}

export function OnboardingScreen({
  apiKey,
  setApiKey,
  connecting,
  changingKey,
  hasApiKey,
  error,
  onSubmit,
  onBack,
  onForget,
  onOpenLibrary,
}: OnboardingScreenProps) {
  return (
    <main className="onboarding-shell">
      <header className="onboarding-titlebar" data-tauri-drag-region>
        <BrandMark />
      </header>
      <section className="onboarding-manifesto" aria-labelledby="welcome-title">
        <div className="manifesto-copy">
          <p className="eyebrow">OpenRouter image generation, refined.</p>
          <h1 id="welcome-title">
            A beautiful front end
            <br />
            for powerful models.
          </h1>
          <p>
            Eidos is a free desktop workspace for OpenRouter image generation.
            No Eidos account or subscription—bring your API key, choose a
            model, and start creating.
          </p>
        </div>
      </section>

      <section className="credential-panel">
        <form className="credential-form" onSubmit={onSubmit}>
          <div>
            <h2>{changingKey ? "Replace your key" : "Bring your own key"}</h2>
            <p className="form-intro">
              Eidos uses your OpenRouter balance directly. There is no Eidos
              account and no separate subscription.
            </p>
          </div>

          <label className="field-label" htmlFor="api-key">
            OpenRouter API key
          </label>
          <div className="key-field-wrap">
            <KeyIcon />
            <input
              id="api-key"
              type="password"
              value={apiKey}
              onChange={(event) => setApiKey(event.currentTarget.value)}
              placeholder="sk-or-v1-…"
              autoComplete="off"
              spellCheck={false}
              autoFocus
            />
          </div>

          {error && <ErrorMessage error={error} compact />}

          <button
            className="primary-button connect-button"
            type="submit"
            disabled={connecting || apiKey.trim().length < 20}
          >
            <span>{connecting ? "Checking key" : "Connect OpenRouter"}</span>
            {connecting ? <Spinner /> : <ArrowIcon />}
          </button>

          {!hasApiKey && (
            <button
              className="offline-entry-button"
              type="button"
              onClick={onOpenLibrary}
              disabled={connecting}
            >
              <span aria-hidden="true" />
              Open local library
            </button>
          )}

          <div className="credential-actions">
            {changingKey && hasApiKey && (
              <button
                className="text-button"
                type="button"
                onClick={onBack}
                disabled={connecting}
              >
                <BackIcon />
                Back to studio
              </button>
            )}
            {hasApiKey && (
              <button
                className="text-button danger-text"
                type="button"
                onClick={onForget}
                disabled={connecting}
              >
                <TrashIcon />
                Forget stored key
              </button>
            )}
          </div>

          <p className="privacy-note">
            <LockIcon />
            Stored in macOS Keychain. When you generate, prompts and references
            are sent to OpenRouter and the model provider.
          </p>
        </form>
      </section>
    </main>
  );
}
