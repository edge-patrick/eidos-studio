import { BrandMark } from "./BrandMark";

export type StudioView = "create" | "library";

interface StudioHeaderProps {
  activeView: StudioView;
  generating: boolean;
  connected: boolean;
  onViewChange: (view: StudioView) => void;
  onChangeKey: () => void;
}

export function StudioHeader({
  activeView,
  generating,
  connected,
  onViewChange,
  onChangeKey,
}: StudioHeaderProps) {
  return (
    <header className="studio-header" data-tauri-drag-region>
      <BrandMark />

      <nav className="studio-view-switcher" aria-label="Studio views">
        <button
          className={activeView === "create" ? "active" : ""}
          type="button"
          aria-current={activeView === "create" ? "page" : undefined}
          onClick={() => onViewChange("create")}
        >
          Create
          {generating && (
            <span
              className="view-generation-dot"
              aria-label="Generation in progress"
            />
          )}
        </button>
        <button
          className={activeView === "library" ? "active" : ""}
          type="button"
          aria-current={activeView === "library" ? "page" : undefined}
          onClick={() => onViewChange("library")}
        >
          Library
        </button>
      </nav>

      <button
        className={`key-status-button${connected ? "" : " disconnected"}`}
        type="button"
        onClick={onChangeKey}
        disabled={generating}
      >
        <span className="openrouter-status-dot" aria-hidden="true" />
        <span className="key-status-copy">
          <strong>OpenRouter {connected ? "connected" : "not connected"}</strong>
          <small>{connected ? "Click to change key" : "Click to connect"}</small>
        </span>
      </button>
    </header>
  );
}
