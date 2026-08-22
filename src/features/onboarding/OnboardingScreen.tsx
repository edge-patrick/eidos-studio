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
  apiKeyPreview: string | null;
  setApiKey: (value: string) => void;
  connecting: boolean;
  changingKey: boolean;
  hasApiKey: boolean;
  error: AppError | null;
  onSubmit: (event: React.FormEvent) => void;
  onBack: () => void;
  onChangeKey: () => void;
  onCancelChange: () => void;
  onForget: () => void;
  onOpenLibrary: () => void;
}

export function OnboardingScreen({
  apiKey,
  apiKeyPreview,
  setApiKey,
  connecting,
  changingKey,
  hasApiKey,
  error,
  onSubmit,
  onBack,
  onChangeKey,
  onCancelChange,
  onForget,
  onOpenLibrary,
}: OnboardingScreenProps) {
  const showingStoredKey = hasApiKey && !changingKey;

  return (
    <main className="onboarding-shell">
      <header className="onboarding-titlebar" data-tauri-drag-region>
        <BrandMark />
      </header>
      <section className="onboarding-manifesto" aria-labelledby="welcome-title">
        <div className="manifesto-copy">
          <p className="eyebrow">OpenRouter image generation, refined.</p>
          <h1 id="welcome-title">
            Create images.
            <br />
            With every model.
            <br />
            In one place.
          </h1>
          <p>
            Eidos is a free desktop workspace for OpenRouter image generation.
            No Eidos account or subscription—bring your API key, choose a
            model, and start creating.
          </p>
        </div>
      </section>

      <section className="credential-panel">
        {showingStoredKey ? (
          <div className="credential-form stored-key-state">
            <div>
              <h2>OpenRouter connected</h2>
              <p className="form-intro">
                Your saved key is ready to use with your OpenRouter balance.
              </p>
            </div>

            <label className="field-label" htmlFor="stored-api-key">
              Stored OpenRouter API key
            </label>
            <div className="key-field-wrap is-disabled">
              <KeyIcon />
              <input
                id="stored-api-key"
                type="text"
                value={apiKeyPreview ?? "sk-or-v1-••••••••••••••••"}
                disabled
                readOnly
              />
            </div>

            {error && <ErrorMessage error={error} compact />}

            <button
              className="primary-button connect-button"
              type="button"
              onClick={onChangeKey}
              disabled={connecting}
            >
              <span>Change key</span>
              <ArrowIcon />
            </button>

            <div className="credential-actions">
              <button
                className="text-button"
                type="button"
                onClick={onBack}
                disabled={connecting}
              >
                <BackIcon />
                Back to studio
              </button>
              <button
                className="text-button danger-text"
                type="button"
                onClick={onForget}
                disabled={connecting}
              >
                <TrashIcon />
                Forget stored key
              </button>
            </div>

            <p className="privacy-note">
              <LockIcon />
              Stored in macOS Keychain. When you generate, prompts and references
              are sent to OpenRouter and the model provider.
            </p>
          </div>
        ) : (
          <form className="credential-form" onSubmit={onSubmit}>
            <div>
              <h2>{changingKey ? "Connect a new key" : "Bring your own key"}</h2>
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

            {changingKey ? (
              <div className="credential-actions replacement-actions">
                <button
                  className="text-button"
                  type="button"
                  onClick={onCancelChange}
                  disabled={connecting}
                >
                  <BackIcon />
                  Keep current key
                </button>
              </div>
            ) : (
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

            <p className="privacy-note">
              <LockIcon />
              Stored in macOS Keychain. When you generate, prompts and references
              are sent to OpenRouter and the model provider.
            </p>
          </form>
        )}
      </section>
    </main>
  );
}
