# Eidos Studio — Implementation Roadmap

## Product direction

Build the smallest useful image-generation app first, then expand it without replacing the core architecture.

Project data is stored locally. Prompts and reference images are sent to OpenRouter and the selected provider for generation.

## Plan 1 — Nano Banana MVP

### Goal

A user can enter an OpenRouter key, write a prompt, optionally attach one reference image, generate with Gemini Nano Banana, and save the result.

Use one fixed model:

```text
google/gemini-3.1-flash-image
```

Keep the model slug in one Rust configuration constant so it can be replaced later.

### User flow

1. Enter and validate an OpenRouter API key on first launch.
2. Enter a prompt on the main screen.
3. Optionally choose one local reference image.
4. Generate an image.
5. View the result.
6. Save the result or retry the request.

### Interface

- API-key onboarding screen
- Prompt field
- Optional reference-image picker and preview
- Generate and Cancel buttons
- Loading state
- Result preview
- Save As, Retry, and New Generation actions
- Clear error message with optional sanitized details

### Technical foundation

- Tauri 2, React, TypeScript, and Vite
- Rust core for credentials, OpenRouter requests, files, and SQLite
- API key stored in the operating system credential store
- API key never stored in SQLite or returned to the frontend
- Background generation jobs with request IDs, cancellation tokens, and terminal events
- Buffered provider response with strict limits; image bytes remain in Rust and previews use managed asset paths
- Local output files with metadata stored in SQLite
- Database migrations from the first version

### Initial database

`generation_attempts`:

- ID
- Prompt
- Model ID
- Status: running, succeeded, failed, or cancelled
- Created and completed timestamps
- Cost
- Error category and sanitized message

`assets` (content-addressed objects):

- ID
- Content hash
- Portable path relative to the managed asset root
- MIME type
- Width and height

`attempt_assets` (attempt-to-object links):

- Generation ID
- Asset ID
- Role: reference or output
- Sort order for future multi-image support

There is no history screen yet. Attempts are recorded so one can be added without redesigning storage.

### Build order

1. Scaffold the application and test commands.
2. Add SQLite and migrations.
3. Implement secure API-key onboarding.
4. Implement the fixed-model OpenRouter request in Rust.
5. Add prompt and reference-image input.
6. Add generation, cancellation, result display, retry, and saving.
7. Normalize essential errors.
8. Audit secret handling and verify a macOS development build.

### Done when

- A fresh user can connect a valid key and generate an image.
- Text-only and one-reference-image requests work.
- The result can be saved locally.
- The key survives restart without appearing in preferences, SQLite, or logs.
- Successful, failed, and cancelled attempts are recorded correctly.
- Errors provide a useful Retry action.

### Excluded from Plan 1

- Model selection
- Generation settings
- Multiple references or outputs
- History interface
- Model comparison
- Provider routing controls
- Streaming previews
- Telemetry
- Windows and Linux release work

## Plan 2 — Local library and workflow polish

### Goal

Turn recorded attempts into a useful local image library.

### Scope

- History screen and generation detail view
- Successful, failed, and cancelled attempt filtering
- Multiple reference images with drag, drop, removal, and ordering
- Copy, drag, reveal, export, retry, and duplicate actions
- Managed local asset storage
- History deletion and disable-history option
- Missing-file and database-recovery handling
- Improved keyboard navigation and accessibility

### Done when

- Users can revisit, reuse, export, and delete previous work reliably.
- Moving an original reference file does not break a saved generation.

## Plan 3 — Models and generation settings

### Goal

Add multiple image models without hardcoding their controls.

### Scope

- Runtime OpenRouter image-model discovery
- Per-provider endpoint capability and pricing discovery
- Cached curated model list with manual refresh
- Model and provider selection
- Capability-aware aspect ratio, resolution, quality, output format, background, seed, and output-count controls
- Provider routing and fallback visibility
- Estimated price before generation and actual cost afterward
- Provider-specific error metadata

### Current implementation slice

- Curated Gemini Flash Lite, Flash, and Pro models, OpenAI GPT Image 2, and the complete FLUX.2 image family, refreshed with normalized OpenRouter capabilities at runtime
- Model selection with capability-aware aspect ratio, resolution, and reference limits
- Per-model setting drafts and exact model restoration from local history
- Exact selected model persisted for every attempt
- Explicit Google AI Studio routing for Gemini Pro 4K requests

Pricing discovery, manual catalog refresh, advanced settings, endpoint visibility, and general provider selection remain for later slices.

### Done when

- Unsupported settings cannot be selected.
- Every attempt records the exact model, endpoint, capabilities, and settings used.

## Plan 4 — Comparison workspace

### Goal

Make model differences the central product feature.

### Scope

- Run the same prompt and references through two or more models
- Side-by-side results
- Cost, duration, provider, settings, and error comparison
- Retry only one comparison candidate
- Promote a result into a new generation
- Save comparison groups in the local library

### Done when

- A user can evaluate model output, cost, speed, and refusal behavior in one view.

## Plan 5 — Public release

### Goal

Ship a secure and dependable open-source macOS release, then validate other platforms.

### Scope

- Final visual identity and application icon
- Tauri permission, file-access, CSP, and secret-handling audit
- Offline, timeout, malformed-response, and low-credit testing
- Light and dark appearance polish
- macOS signing and notarization
- License, contributor guide, privacy explanation, and release workflow
- Windows and Linux build verification
- Optional privacy controls such as Zero Data Retention routing

### Done when

- A clean Mac can install and run the signed application.
- Privacy boundaries and remote processing are explained clearly.
- Windows and Linux are only advertised after platform testing passes.
