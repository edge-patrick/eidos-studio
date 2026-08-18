import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { ErrorMessage } from "../../components/ErrorMessage";
import {
  ArrowIcon,
  CloseIcon,
  CopyIcon,
  DownloadIcon,
  ExpandIcon,
  Spinner,
  TrashIcon,
  WarningIcon,
} from "../../components/Icons";
import { eidosApi, normalizeError } from "../../shared/eidosApi";
import type {
  AppError,
  AppStatus,
  HistoryAsset,
  HistoryAttempt,
  HistoryCursor,
} from "../../shared/types";

const HISTORY_PAGE_SIZE = 60;

interface LibraryScreenProps {
  active: boolean;
  status: AppStatus;
  generationBusy: boolean;
  onReuse: (attempt: HistoryAttempt) => Promise<void>;
  onDeleted: (attemptId: string) => void;
}

export function LibraryScreen({
  active,
  status,
  generationBusy,
  onReuse,
  onDeleted,
}: LibraryScreenProps) {
  const [attempts, setAttempts] = useState<HistoryAttempt[]>([]);
  const attemptsRef = useRef<HistoryAttempt[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [nextCursor, setNextCursor] = useState<HistoryCursor | null>(null);
  const [totalCount, setTotalCount] = useState(0);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  const [refreshError, setRefreshError] = useState<AppError | null>(null);

  const loadFirstPage = useCallback(async (showLoading = false) => {
    if (showLoading && attemptsRef.current.length === 0) setLoading(true);
    try {
      const page = await eidosApi.listHistory(null, HISTORY_PAGE_SIZE);
      const preserveLoadedPages = attemptsRef.current.length > HISTORY_PAGE_SIZE;
      const firstPageIds = new Set(page.attempts.map((attempt) => attempt.id));
      const merged = preserveLoadedPages
        ? [
            ...page.attempts,
            ...attemptsRef.current.filter((attempt) => !firstPageIds.has(attempt.id)),
          ]
        : page.attempts;
      attemptsRef.current = merged;
      setAttempts(merged);
      setSelectedId((selected) =>
        selected && merged.some((attempt) => attempt.id === selected) ? selected : null,
      );
      if (!preserveLoadedPages) setNextCursor(page.nextCursor ?? null);
      setTotalCount(page.totalCount);
      setError(null);
      setRefreshError(null);
    } catch (nextError) {
      const normalized = normalizeError(nextError);
      if (attemptsRef.current.length === 0) setError(normalized);
      else setRefreshError(normalized);
    } finally {
      setLoading(false);
    }
  }, []);

  const loadMore = useCallback(async () => {
    if (!nextCursor || loadingMore) return;
    const cursor = nextCursor;
    setLoadingMore(true);
    try {
      const page = await eidosApi.listHistory(cursor, HISTORY_PAGE_SIZE);
      const knownIds = new Set(attemptsRef.current.map((attempt) => attempt.id));
      const merged = [
        ...attemptsRef.current,
        ...page.attempts.filter((attempt) => !knownIds.has(attempt.id)),
      ];
      attemptsRef.current = merged;
      setAttempts(merged);
      setNextCursor(page.nextCursor ?? null);
      setTotalCount(page.totalCount);
      setRefreshError(null);
    } catch (nextError) {
      setRefreshError(normalizeError(nextError));
    } finally {
      setLoadingMore(false);
    }
  }, [loadingMore, nextCursor]);

  useEffect(() => {
    if (!active) return;
    void loadFirstPage(true);
  }, [active, loadFirstPage]);

  useEffect(() => {
    if (!active) return;
    const unlisten = eidosApi.listenToGenerationJobs(() => {
      void loadFirstPage();
    });
    return () => {
      void unlisten.then((stop) => stop()).catch(() => undefined);
    };
  }, [active, loadFirstPage]);

  useEffect(() => {
    if (!active) return;
    const unlisten = eidosApi.listenToHistoryThumbnails(() => {
      void loadFirstPage();
    });
    return () => {
      void unlisten.then((stop) => stop()).catch(() => undefined);
    };
  }, [active, loadFirstPage]);

  useEffect(() => {
    function closeInspector(event: KeyboardEvent) {
      if (event.key === "Escape" && selectedId) setSelectedId(null);
    }
    document.addEventListener("keydown", closeInspector);
    return () => document.removeEventListener("keydown", closeInspector);
  }, [selectedId]);

  const selectedAttempt = useMemo(
    () => attempts.find((attempt) => attempt.id === selectedId) ?? null,
    [attempts, selectedId],
  );

  function removeAttempt(id: string) {
    setAttempts((current) => {
      const filtered = current.filter((attempt) => attempt.id !== id);
      attemptsRef.current = filtered;
      return filtered;
    });
    setTotalCount((current) => Math.max(0, current - 1));
    setSelectedId(null);
    onDeleted(id);
  }

  return (
    <section
      className={`library-screen${selectedAttempt ? " inspector-open" : ""}`}
      aria-label="Generation library"
    >
      <div className="library-main">
        <header className="library-heading">
          <div>
            <p className="eyebrow">Local archive</p>
            <h1>Library</h1>
          </div>
          {!loading && !error && (
            <span className="library-count">
              {totalCount} {totalCount === 1 ? "generation" : "generations"}
            </span>
          )}
        </header>

        {loading ? (
          <LibraryLoading />
        ) : error ? (
          <div className="library-message-state">
            <p className="eyebrow">Archive unavailable</p>
            <h2>History could not be opened.</h2>
            <ErrorMessage error={error} />
            <button
              className="stage-secondary-button"
              type="button"
              onClick={() => void loadFirstPage(true)}
            >
              Try again
            </button>
          </div>
        ) : attempts.length === 0 ? (
          <div className="library-message-state library-empty-state">
            <span className="empty-frame" aria-hidden="true" />
            <p className="eyebrow">Nothing developed yet</p>
            <h2>Your generations will collect here.</h2>
            <p>Successful, failed, and cancelled attempts are kept locally on this Mac.</p>
          </div>
        ) : (
          <>
            {refreshError && (
              <div className="library-refresh-warning" role="status">
                <span>Library refresh paused. Your loaded generations are still available.</span>
                <button type="button" onClick={() => void loadFirstPage()}>
                  Retry
                </button>
              </div>
            )}
            <div className="history-grid">
              {attempts.map((attempt, index) => (
                <HistoryCard
                  key={attempt.id}
                  attempt={attempt}
                  selected={attempt.id === selectedId}
                  index={index}
                  onSelect={() => setSelectedId(attempt.id)}
                />
              ))}
            </div>
            {nextCursor && (
              <div className="history-pagination">
                <span>{attempts.length} of {totalCount} loaded</span>
                <button
                  className="stage-secondary-button"
                  type="button"
                  onClick={() => void loadMore()}
                  disabled={loadingMore}
                >
                  {loadingMore ? (
                    <>
                      <Spinner /> Loading…
                    </>
                  ) : (
                    "Load more"
                  )}
                </button>
              </div>
            )}
          </>
        )}
      </div>

      {selectedAttempt && (
        <HistoryInspector
          key={selectedAttempt.id}
          attempt={selectedAttempt}
          status={status}
          generationBusy={generationBusy}
          onClose={() => setSelectedId(null)}
          onReuse={onReuse}
          onDeleted={() => removeAttempt(selectedAttempt.id)}
        />
      )}
    </section>
  );
}

function HistoryCard({
  attempt,
  selected,
  index,
  onSelect,
}: {
  attempt: HistoryAttempt;
  selected: boolean;
  index: number;
  onSelect: () => void;
}) {
  const succeeded = attempt.status === "succeeded";
  const cancelled = attempt.status === "cancelled";
  return (
    <button
      className={`history-card${selected ? " selected" : ""}`}
      style={{ "--card-index": Math.min(index, 8) } as React.CSSProperties}
      type="button"
      aria-pressed={selected}
      onClick={onSelect}
    >
      <div className="history-card-preview">
        {succeeded && attempt.output ? (
          <LayeredHistoryImage asset={attempt.output} useThumbnail />
        ) : (
          <div className={`failed-card-art${cancelled ? " cancelled" : ""}`}>
            <span><WarningIcon /></span>
            <strong>
              {succeeded
                ? "Image unavailable"
                : cancelled
                  ? "Generation cancelled"
                  : "Generation failed"}
            </strong>
          </div>
        )}
      </div>
      <div className="history-card-copy">
        <time dateTime={attempt.createdAt}>{formatCardDate(attempt.createdAt)}</time>
        <p>{attempt.prompt}</p>
      </div>
    </button>
  );
}

function HistoryInspector({
  attempt,
  status,
  generationBusy,
  onClose,
  onReuse,
  onDeleted,
}: {
  attempt: HistoryAttempt;
  status: AppStatus;
  generationBusy: boolean;
  onClose: () => void;
  onReuse: (attempt: HistoryAttempt) => Promise<void>;
  onDeleted: () => void;
}) {
  const [actionBusy, setActionBusy] = useState<"save" | "reuse" | "delete" | null>(null);
  const [actionError, setActionError] = useState<AppError | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [copied, setCopied] = useState(false);
  const [lightboxOpen, setLightboxOpen] = useState(false);
  const imageButtonRef = useRef<HTMLButtonElement>(null);
  const succeeded = attempt.status === "succeeded";
  const cancelled = attempt.status === "cancelled";

  function closeLightbox() {
    setLightboxOpen(false);
    window.setTimeout(() => imageButtonRef.current?.focus(), 0);
  }

  async function copyPrompt() {
    try {
      await navigator.clipboard.writeText(attempt.prompt);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1800);
    } catch (error) {
      setActionError(normalizeError(error));
    }
  }

  async function saveOutput() {
    setActionBusy("save");
    setActionError(null);
    try {
      const result = await eidosApi.saveOutput(attempt.id);
      if (result) setNotice(`Saved to ${result.path}`);
    } catch (error) {
      setActionError(normalizeError(error));
    } finally {
      setActionBusy(null);
    }
  }

  async function reuseAttempt() {
    setActionBusy("reuse");
    setActionError(null);
    try {
      await onReuse(attempt);
    } catch (error) {
      setActionError(normalizeError(error));
    } finally {
      setActionBusy(null);
    }
  }

  async function deleteAttempt() {
    setActionBusy("delete");
    setActionError(null);
    try {
      const result = await eidosApi.deleteHistoryAttempt(attempt.id);
      if (!result.deleted) {
        throw new Error("This history item no longer exists.");
      }
      onDeleted();
    } catch (error) {
      setActionError(normalizeError(error));
      setActionBusy(null);
      setConfirmDelete(false);
    }
  }

  return (
    <aside className="history-inspector" aria-label="Generation details">
      <header className="inspector-header">
        <div>
          <p className="eyebrow">Generation</p>
          <h2>Details</h2>
        </div>
        <button className="inspector-close" type="button" onClick={onClose} aria-label="Close details">
          <CloseIcon />
        </button>
      </header>

      <div className="inspector-scroll">
        {succeeded && attempt.output ? (
          <button
            ref={imageButtonRef}
            className="inspector-image"
            type="button"
            aria-label="View image full size"
            onClick={() => setLightboxOpen(true)}
          >
            <LayeredHistoryImage asset={attempt.output} />
            <span className="inspector-zoom-cue" aria-hidden="true">
              <ExpandIcon />
              View full size
            </span>
          </button>
        ) : (
          <div className={`inspector-failure${cancelled ? " cancelled" : ""}`}>
            <span><WarningIcon /></span>
            <div>
              <strong>
                {succeeded
                  ? "Image unavailable"
                  : cancelled
                    ? "Generation cancelled"
                    : "Image not made"}
              </strong>
              <p>{attempt.errorMessage ?? "The provider did not return an image."}</p>
            </div>
          </div>
        )}

        <section className="inspector-section prompt-detail">
          <div className="inspector-section-heading">
            <h3>Prompt</h3>
            <button type="button" onClick={() => void copyPrompt()}>
              <CopyIcon />
              {copied ? "Copied" : "Copy"}
            </button>
          </div>
          <p>{attempt.prompt}</p>
        </section>

        {attempt.references.length > 0 && (
          <section className="inspector-section">
            <h3>References</h3>
            <div className="history-reference-list">
              {attempt.references.map((reference, index) => (
                <div className="history-reference" key={`${reference.assetPath}-${index}`}>
                  <HistoryImage
                    asset={reference}
                    alt={`Reference ${index + 1} used for generation`}
                    useThumbnail
                  />
                  <div>
                    <strong>Reference {index + 1}</strong>
                    <span>{reference.width} × {reference.height}</span>
                  </div>
                </div>
              ))}
            </div>
          </section>
        )}

        <section className="inspector-section">
          <h3>Details</h3>
          <dl className="history-metadata">
            <Metadata label="Created" value={formatDetailDate(attempt.createdAt)} />
            <Metadata label="Model" value={modelName(attempt.modelId, status)} />
            {attempt.output && (
              <Metadata label="Size" value={`${attempt.output.width} × ${attempt.output.height}`} />
            )}
            <Metadata label="Settings" value={formatSettings(attempt)} />
            {attempt.providerName && <Metadata label="Provider" value={attempt.providerName} />}
            {typeof attempt.durationMs === "number" && (
              <Metadata label="Duration" value={formatDuration(attempt.durationMs)} />
            )}
            {succeeded && (
              <Metadata
                label="Cost"
                value={typeof attempt.costUsd === "number" ? `$${attempt.costUsd.toFixed(4)}` : "—"}
              />
            )}
          </dl>
        </section>

        {actionError && <ErrorMessage error={actionError} compact />}
        {notice && <p className="inspector-notice">{notice}</p>}
      </div>

      {confirmDelete && (
        <div className="delete-confirmation" role="alert">
          <div>
            <strong>Permanently delete?</strong>
            <span>Unused local image files will also be deleted.</span>
          </div>
          <button type="button" onClick={() => setConfirmDelete(false)} disabled={actionBusy === "delete"}>
            Keep
          </button>
          <button className="confirm-delete-button" type="button" onClick={() => void deleteAttempt()} disabled={actionBusy === "delete"}>
            {actionBusy === "delete" ? "Deleting…" : "Delete"}
          </button>
        </div>
      )}

      <footer className="inspector-actions">
        <button
          className="inspector-delete-action"
          type="button"
          onClick={() => setConfirmDelete(true)}
          disabled={actionBusy !== null || confirmDelete}
        >
          <TrashIcon />
          Delete
        </button>
        {succeeded && attempt.output && (
          <button
            className="stage-secondary-button inspector-save"
            type="button"
            onClick={() => void saveOutput()}
            disabled={actionBusy !== null}
          >
            {actionBusy === "save" ? <Spinner /> : <DownloadIcon />}
            Save as
          </button>
        )}
        <button
          className="result-save-button inspector-primary-action"
          type="button"
          onClick={() => void reuseAttempt()}
          disabled={actionBusy !== null || generationBusy}
          title={generationBusy ? "Wait for the current generation to finish" : undefined}
        >
          {actionBusy === "reuse" ? <Spinner /> : (
            <>{succeeded ? "Use again" : "Edit"}<ArrowIcon /></>
          )}
        </button>
      </footer>

      {lightboxOpen && attempt.output && (
        <ImageLightbox
          asset={attempt.output}
          onClose={closeLightbox}
        />
      )}
    </aside>
  );
}

function ImageLightbox({
  asset,
  onClose,
}: {
  asset: HistoryAsset;
  onClose: () => void;
}) {
  const closeButtonRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    closeButtonRef.current?.focus();
    function closeWithEscape(event: KeyboardEvent) {
      if (event.key === "Tab") {
        event.preventDefault();
        closeButtonRef.current?.focus();
        return;
      }
      if (event.key !== "Escape") return;
      event.preventDefault();
      event.stopImmediatePropagation();
      onClose();
    }
    document.addEventListener("keydown", closeWithEscape, true);
    return () => document.removeEventListener("keydown", closeWithEscape, true);
  }, [onClose]);

  return createPortal(
    <div
      className="history-lightbox"
      role="dialog"
      aria-modal="true"
      aria-label="Full-size generated image"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div className="lightbox-image-wrap">
        <HistoryImage asset={asset} alt="Generated result at full size" />
      </div>
      <button
        ref={closeButtonRef}
        className="lightbox-close"
        type="button"
        onClick={onClose}
        aria-label="Close full-size image"
      >
        <CloseIcon />
      </button>
      <span className="lightbox-hint">Esc to close</span>
    </div>,
    document.body,
  );
}

function LayeredHistoryImage({
  asset,
  useThumbnail = false,
}: {
  asset: HistoryAsset;
  useThumbnail?: boolean;
}) {
  const preferredPath = useThumbnail && asset.thumbnailPath
    ? asset.thumbnailPath
    : asset.assetPath;
  const [sourcePath, setSourcePath] = useState(preferredPath);
  const [missing, setMissing] = useState(false);

  useEffect(() => {
    setSourcePath(preferredPath);
    setMissing(false);
  }, [preferredPath]);

  if (missing) {
    return <span className="missing-image"><WarningIcon /> Image unavailable</span>;
  }
  const source = eidosApi.assetUrl(sourcePath);

  function handleError() {
    if (sourcePath !== asset.assetPath) {
      setSourcePath(asset.assetPath);
    } else {
      setMissing(true);
    }
  }

  return (
    <>
      <img
        className="history-image-blur"
        src={source}
        alt=""
        aria-hidden="true"
        loading="lazy"
        draggable={false}
      />
      <img
        className="history-image-fit"
        src={source}
        alt=""
        loading="lazy"
        draggable={false}
        onError={handleError}
      />
    </>
  );
}

function HistoryImage({
  asset,
  alt,
  useThumbnail = false,
}: {
  asset: HistoryAsset;
  alt: string;
  useThumbnail?: boolean;
}) {
  const preferredPath = useThumbnail && asset.thumbnailPath
    ? asset.thumbnailPath
    : asset.assetPath;
  const [sourcePath, setSourcePath] = useState(preferredPath);
  const [missing, setMissing] = useState(false);

  useEffect(() => {
    setSourcePath(preferredPath);
    setMissing(false);
  }, [preferredPath]);

  if (missing) {
    return <span className="missing-image"><WarningIcon /> Image unavailable</span>;
  }
  return (
    <img
      src={eidosApi.assetUrl(sourcePath)}
      alt={alt}
      loading="lazy"
      decoding="async"
      draggable={false}
      onError={() => {
        if (sourcePath !== asset.assetPath) {
          setSourcePath(asset.assetPath);
        } else {
          setMissing(true);
        }
      }}
    />
  );
}

function Metadata({ label, value }: { label: string; value: string }) {
  return <div><dt>{label}</dt><dd>{value}</dd></div>;
}

function LibraryLoading() {
  return (
    <div className="history-grid history-loading" aria-label="Loading history">
      {Array.from({ length: 8 }, (_, index) => (
        <div className="history-card-skeleton" key={index} aria-hidden="true">
          <span /><i /><i />
        </div>
      ))}
    </div>
  );
}

function formatCardDate(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.valueOf())) return value;
  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}

function formatDetailDate(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.valueOf())) return value;
  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}

function formatSettings(attempt: HistoryAttempt) {
  return `${attempt.settings.aspectRatio ?? "Auto"} · ${attempt.settings.resolution ?? "Auto"}`;
}

function formatDuration(durationMs: number) {
  return durationMs < 1000 ? `${durationMs}ms` : `${(durationMs / 1000).toFixed(1)}s`;
}

function modelName(modelId: string, status: AppStatus) {
  if (modelId === status.modelId) return status.modelName;
  const modelPath = modelId.split("/");
  return modelPath[modelPath.length - 1] ?? modelId;
}
