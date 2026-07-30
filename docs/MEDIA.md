# Local media generation

Optional. Nothing in the workbench needs it. It exists so an agent that needs an image, a
voice-over, a short clip or a transcript produces it **on your machine** instead of reaching for
a paid cloud API — which also means the input never leaves the machine.

The agent roles carry a matching rule: media is generated locally by default, and a cloud
service is used only when local quality is demonstrably not enough for the concrete task — and
then it says so.

## What the four commands do

| Command | What it does | Rough cost |
|---|---|---|
| `bild "a red apple on a wooden table"` | image generation, several backends and a fast/quality/text-in-image switch | ~10 s fast, ~1 min quality, ~3 min for the text-accurate model |
| `video "waves breaking at sunset"` | short video, also image-to-video (`--bild photo.png`) | minutes; the quality mode noticeably longer |
| `tts "some text"` | text to speech, several voices and an expressive mode | seconds |
| `stt recording.wav` | speech to text, with a timestamped fallback engine | seconds to a minute |

Each has `--help`. Run it before assuming a flag exists.

## Hardware, honestly

This is the part that does not run everywhere.

- **Apple Silicon** is the shipped path: `bin/` uses MLX (mflux for images, mlx-audio for speech).
  Roughly 16 GB unified memory gets you images; the large text-accurate image model wants ~34 GB;
  video wants everything you have and it is still slow.
- **Linux with an NVIDIA GPU** uses `bin-linux/`, built around CUDA and whisper.cpp. A 12 GB card
  handles images and speech comfortably; video is out of reach there.
- **Anything else**: skip this layer. The workbench does not care.

Only one large model at a time. Before starting any of them, run `check-resources` — and stop a
loaded model with `ollama stop <model>` before generating video, or the machine will swap itself
to a standstill.

## What must be installed separately

These are large downloads with their own licenses, so they are not in the repo:

| Component | For | How |
|---|---|---|
| mflux | images on Apple Silicon | `pip install mflux` |
| mlx-audio | speech on Apple Silicon | `pip install mlx-audio` |
| parakeet-mlx | fast transcription | `pip install parakeet-mlx` |
| whisper.cpp | transcription on Linux/CUDA, timestamped fallback | build from source |
| The model weights themselves | all of the above | downloaded on first run, several GB each |

Draw Things (macOS, free) is a graphical alternative for images and shares the same model files.

## `medien-ui`

A small local interface over the same four commands, for when clicking is faster than typing
flags. It runs against the same models; nothing is duplicated.
