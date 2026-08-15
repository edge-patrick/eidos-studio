# Eidos Studio

Eidos Studio is a simple desktop app for generating images with AI using your `OpenRouter` credits. No accounts, sign-in, ads, tracking, or subscriptions. This is an early first version.

## Supported models

- google/gemini-3.1-flash-image

## What it can do

- Generate an image from a text prompt
- Use one local image as a reference
- Choose an aspect ratio and 1K, 2K, or 4K resolution
- Keep generation history and image files locally

Prompts and reference images stay in the local Eidos library, but they are sent to OpenRouter and Google when you generate an image.

## Roadmap

- [x] Text-to-image generation
- [x] Reference images
- [x] Local generation history
- [x] Aspect ratio and resolution controls
- [ ] A full history and image library
- [ ] Multiple reference images
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
