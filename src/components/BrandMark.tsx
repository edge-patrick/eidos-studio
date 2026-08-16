import eidosLogo from "../assets/eidos-logo.svg";
import packageJson from "../../package.json";

export function BrandMark() {
  return (
    <div className="brand-mark" aria-label="Eidos Studio">
      <span className="brand-glyph" aria-hidden="true">
        <img src={eidosLogo} alt="" draggable={false} />
      </span>
      <span className="brand-copy">
        <strong>Eidos Studio</strong>
        <small>Version {packageJson.version}</small>
      </span>
    </div>
  );
}
