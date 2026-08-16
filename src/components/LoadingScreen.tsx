import { BrandMark } from "./BrandMark";

export function LoadingScreen() {
  return (
    <main
      className="boot-screen"
      aria-label="Opening Eidos Studio"
      data-tauri-drag-region
    >
      <BrandMark />
      <div className="boot-line" />
      <p>Preparing the studio</p>
    </main>
  );
}
