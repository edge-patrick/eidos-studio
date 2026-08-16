import type { AppStatus } from "../../shared/types";
import { ComposerPanel } from "./ComposerPanel";
import { ResultPanel } from "./ResultPanel";
import type { StudioController } from "./useStudioController";

interface WorkspaceScreenProps {
  status: AppStatus;
  studio: StudioController;
}

export function WorkspaceScreen({
  status,
  studio,
}: WorkspaceScreenProps) {
  return (
    <>
      <div className="studio-grid">
        <ComposerPanel status={status} studio={studio} />
        <ResultPanel studio={studio} />
      </div>

      {studio.notice && (
        <div className="notice-toast" role="status">
          <span />
          {studio.notice}
        </div>
      )}
    </>
  );
}
