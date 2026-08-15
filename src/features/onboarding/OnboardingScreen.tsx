import { BrandMark } from "../../components/BrandMark";
import { ErrorMessage } from "../../components/ErrorMessage";
import { ArrowIcon, KeyIcon, LockIcon, Spinner } from "../../components/Icons";
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
}: OnboardingScreenProps) {
  return (
    <main className="onboarding-shell">
      <section className="onboarding-manifesto" aria-labelledby="welcome-title">
        <div className="manifesto-topline">
          <BrandMark />
          <span>Local image workspace</span>
        </div>
        <div className="manifesto-copy">
          <p className="eyebrow">One model. One clear surface.</p>
          <h1 id="welcome-title">
            Make the image.
            <br />
            Keep the work.
          </h1>
          <p>
            Eidos keeps your working library on this Mac while Nano Banana
            handles the generation through OpenRouter.
          </p>
        </div>
        <div className="manifesto-index" aria-hidden="true">
          <span>EI</span>
          <span>01</span>
        </div>
      </section>

      <section className="credential-panel">
        <form className="credential-form" onSubmit={onSubmit}>
          <div>
            <p className="section-number">01 / CONNECT</p>
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

          <div className="credential-actions">
            {changingKey && hasApiKey && (
              <button
                className="text-button"
                type="button"
                onClick={onBack}
                disabled={connecting}
              >
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
                Forget stored key
              </button>
            )}
          </div>

          <p className="privacy-note">
            <LockIcon />
            Stored in macOS Keychain. Prompts and references are sent to
            OpenRouter and Google only when you generate.
          </p>
        </form>
      </section>
    </main>
  );
}
