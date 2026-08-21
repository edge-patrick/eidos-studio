import flux from "../../assets/flux.png";
import gptImage from "../../assets/gpt-image.png";
import nanoBananaLite from "../../assets/nano-banana-lite.png";
import nanoBananaPro from "../../assets/nano-banana-pro.png";
import nanoBananaRegular from "../../assets/nano-banana-regular.png";

const modelIcons: Record<string, string> = {
  "black-forest-labs/flux.2-klein-4b": flux,
  "black-forest-labs/flux.2-pro": flux,
  "black-forest-labs/flux.2-flex": flux,
  "black-forest-labs/flux.2-max": flux,
  "openai/gpt-image-2": gptImage,
  "google/gemini-3.1-flash-lite-image": nanoBananaLite,
  "google/gemini-3.1-flash-image": nanoBananaRegular,
  "google/gemini-3-pro-image": nanoBananaPro,
};

export function ModelIcon({ modelId }: { modelId: string }) {
  const icon = modelIcons[modelId];
  return (
    <span className="model-icon" aria-hidden="true">
      {icon ? (
        <img src={icon} alt="" draggable={false} />
      ) : (
        <span className="model-icon-placeholder">◇</span>
      )}
    </span>
  );
}
