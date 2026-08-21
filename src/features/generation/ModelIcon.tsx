import flux from "../../assets/flux.png";
import gptImage from "../../assets/gpt-image.png";
import krea from "../../assets/krea.png";
import nanoBananaLite from "../../assets/nano-banana-lite.png";
import nanoBananaPro from "../../assets/nano-banana-pro.png";
import nanoBananaRegular from "../../assets/nano-banana-regular.png";
import qwen from "../../assets/qwen.png";
import seedream from "../../assets/seedream.png";

const modelIcons: Record<string, string> = {
  "black-forest-labs/flux.2-klein-4b": flux,
  "black-forest-labs/flux.2-pro": flux,
  "black-forest-labs/flux.2-flex": flux,
  "black-forest-labs/flux.2-max": flux,
  "openai/gpt-image-2": gptImage,
  "google/gemini-3.1-flash-lite-image": nanoBananaLite,
  "google/gemini-3.1-flash-image": nanoBananaRegular,
  "google/gemini-3-pro-image": nanoBananaPro,
  "bytedance-seed/seedream-5-0-pro": seedream,
  "qwen/qwen-image-3-pro": qwen,
  "krea/krea-2-medium-turbo": krea,
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
