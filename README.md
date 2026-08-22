![Eidos Studio](./docs/assets/eidos-studio-banner.png)

[![CI](https://github.com/edge-patrick/eidos-studio/actions/workflows/ci.yml/badge.svg)](https://github.com/edge-patrick/eidos-studio/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![macOS](https://img.shields.io/badge/platform-macOS-black?logo=apple)](#run-it-locally)
[![Tauri 2](https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri&logoColor=white)](https://v2.tauri.app/)

# Eidos Studio

Eidos Studio is a local-first desktop app for generating images with AI using your `OpenRouter` credits. No accounts, sign-in, ads, tracking, or subscriptions.

<p align="center">
  <img src="./docs/assets/eidos-studio-interface.png" alt="Eidos Studio app interface" width="680">
</p>

## Why?

If you are a designer, marketer, or copywriter, you may only need AI-generated
images here and there. At current 1K prices, you would need to generate about
300 images every month to make a $20 Google AI Pro subscription worth it for
image generation alone.

| | Eidos Studio | Google AI Pro |
| --- | --- | --- |
| Billing | ✅ Pay only when you generate | ❌ Pay about $20 every month |
| 50 images | ✅ About $3.35 | ❌ About $20 for the plan |
| Output controls | ✅ Choose aspect ratio, 1K, 2K, or 4K | ❌ Fewer explicit controls |
| Model choice | ✅ Choose across Google, OpenAI, Black Forest Labs, ByteDance, Qwen, Krea, Recraft, and xAI models | ❌ Gemini only |
| Cost visibility | ✅ See the exact cost of every image | ❌ No per-image dollar cost |
| Storage | ✅ Keep your library locally on your Mac | ❌ Tied to your Google account |
| Source code | ✅ Free and open source | ❌ Closed source |

*Estimate based on current 1K [Gemini API pricing](https://ai.google.dev/gemini-api/docs/pricing); input and OpenRouter credit-purchase fees are extra.*

## Supported models

- `google/gemini-3.1-flash-lite-image` — Nano Banana 2 Lite (1K)
- `google/gemini-3.1-flash-image` — Nano Banana 2 (1K, 2K, or 4K)
- `google/gemini-3-pro-image` — Nano Banana Pro (1K, 2K, or 4K)
- `openai/gpt-image-2` — GPT Image 2 (capability-aware sizing and references)
- `openai/gpt-image-1-mini` — GPT Image 1 Mini (cost-efficient generation and editing)
- `black-forest-labs/flux.2-klein-4b` — FLUX.2 Klein 4B (fast, cost-efficient generation)
- `black-forest-labs/flux.2-pro` — FLUX.2 Pro (balanced production quality)
- `black-forest-labs/flux.2-flex` — FLUX.2 Flex (typography and fine detail)
- `black-forest-labs/flux.2-max` — FLUX.2 Max (top-tier quality and consistency)
- `bytedance-seed/seedream-5-0-pro` — Seedream 5.0 Pro (commercial visuals and precise editing)
- `bytedance-seed/seedream-5-0-lite` — Seedream 5.0 Lite (fast 2K and 4K exploration)
- `qwen/qwen-image-3-pro` — Qwen Image 3 Pro (typography and fine detail)
- `qwen/qwen-image-3` — Qwen Image 3 (cost-efficient typography and editing)
- `krea/krea-2-medium-turbo` — Krea 2 Medium Turbo (rapid graphic-design exploration)
- `krea/krea-2-medium` — Krea 2 Medium (illustration, anime, and expressive styles)
- `recraft/recraft-v4.1` — Recraft V4.1 (aesthetic concepts and polished design work)
- `x-ai/grok-imagine-image-2.0` — Grok Imagine Image 2.0 (photoreal generation with quality control)

## What it can do

- Generate an image from a text prompt
- Use up to 14 ordered local images as references (12 MB each, 48 MB combined)
- Switch models while keeping model-specific settings separate
- Choose which models appear in the model picker
- Choose a supported aspect ratio and resolution for the selected model
- Choose supported quality levels for models that expose them
- Browse successful, failed, and cancelled generations in a local image library
- Reuse prompts, settings, and reference images from earlier generations
- Save generated images elsewhere or permanently delete them from local storage
- Open the local library without connecting an API key

Prompts and reference images stay in the local Eidos library, but they are sent to OpenRouter and the model provider when you generate an image.

## What's new in 0.4.3

Version 0.4.3 adds Seedream 5.0 Pro, Qwen Image 3 Pro, and Krea 2 Medium Turbo with capability-aware size and reference controls, model icons, and estimated 1K pricing.

## Roadmap

- [x] Text-to-image generation
- [x] Reference images
- [x] Local generation history
- [x] Aspect ratio and resolution controls
- [x] Polished interface and refreshed branding
- [x] A full history and image library
- [x] Multiple reference images
- [x] Multiple Google, OpenAI, Black Forest Labs, ByteDance, Qwen, Krea, Recraft, and xAI models with capability-aware settings
- [x] Model catalog management with persistent picker visibility
- [ ] More image models and settings
- [ ] Side-by-side model comparison
- [ ] More ways to save and share generated images
- [ ] Keyboard shortcuts and accessibility improvements
- [ ] Signed macOS release
- [ ] Windows and Linux support

## Run it locally

You will need Node.js, pnpm, Rust, and Xcode or the Xcode Command Line Tools.

```bash
pnpm install
pnpm tauri dev
```

When the app opens, enter your OpenRouter API key and start creating.

## Built with

- Tauri 2
- React and TypeScript
- Rust
- SQLite
- OpenRouter
