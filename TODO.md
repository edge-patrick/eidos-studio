# TODO

Development notes and follow-ups that are not yet part of the public roadmap.

## Provider and API support

- [ ] Revisit transparent-background support for GPT Image 2 on OpenRouter.
  - OpenAI currently offers transparent backgrounds in preview for `gpt-image-2`
    with `background: "transparent"` and a PNG or WebP output format.
  - As of 2026-08-21, OpenRouter offers the model as `openai/gpt-image-2`, but
    its live capability record only advertises `auto` and `opaque` background
    values. Do not expose the transparent-background option for this model yet.
  - When OpenRouter adds `transparent` to the model's capabilities, add the
    corresponding output control and ensure the output format is PNG or WebP.
  - References:
    [OpenAI image generation guide](https://developers.openai.com/api/docs/guides/image-generation#customize-image-output),
    [OpenRouter image API guide](https://openrouter.ai/docs/guides/overview/multimodal/image-generation),
    [OpenRouter GPT Image 2 capabilities](https://openrouter.ai/api/v1/images/models/openai/gpt-image-2/endpoints)
