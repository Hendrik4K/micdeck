# Third-party components and licences

MicDeck only uses code and knowledge whose licence allows it. This file lists what comes from where
and what was changed.

## DJI-Mic-Control – protocol crate

- Source: <https://github.com/ShadowBitBasher/DJI-Mic-Control>, commit `9ba7688` (2026-07-11)
- Licence: The Unlicense (public domain), text in `crates/dji-protocol/LICENSE`
- Taken over: `crates/protocol` → `crates/dji-protocol`, `PROTOCOL.md` → `docs/protocol/PROTOCOL.md`
- **Not** taken over: the device images in `gui/src/assets/devices/` (official DJI renderings with the
  DJI logo), the app name and icons, the whole interface and the USB layer.

MicDeck's changes to `dji-protocol` (each marked "MicDeck change" in the code):

| File | Change | Reason |
|---|---|---|
| `packet.rs` | `Ack`, `parse_ack()`: reads sequence number and result code (`0x00` accepted, `0xff` refused) | the original doesn't evaluate device replies (DJI-Mic-Control issue #2) |
| `state.rs` | `RxInfo::product_name` | tell receivers with the same USB ID apart |
| `models/mic_mini.rs` | identity record tag `0x06` for unit 0 → receiver product name | as above |
| `models/mic_mini.rs` | a level record no longer creates a transmitter the status push reports as disconnected | avoided a switched-off transmitter briefly showing as connected |
| `models/mic_mini.rs`, `state.rs` | v1: transmitter flags bit `0x02` → `charging` (in its charging case) | observed on real hardware (firmware 01.01.00.56); not decoded upstream |
| tests | new tests for all of the above; one test adapted to the changed order | |

## dji-mic-mo – reference for device variants

- Source: <https://github.com/usokawa/dji-mic-mo>, commit `7c2dd5b` (2026-08-26)
- Used as a reference for field offsets, command IDs and USB IDs of further models. No code was
  copied; facts taken from it are treated as unconfirmed until tested on real hardware.
- Licence (BSD-2-Clause):

```
Copyright (c) 2026 usokawa_.

Redistribution and use in source and binary forms, with or without modification, are permitted
provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this list of conditions
   and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice, this list of
   conditions and the following disclaimer in the documentation and/or other materials provided
   with the distribution.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR
IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND
FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR
CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER
IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT
OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

## Libraries

| Library | Licence | Purpose |
|---|---|---|
| tauri 2 | MIT / Apache-2.0 | app framework |
| nusb 0.2 | MIT / Apache-2.0 | USB (without libusb) |
| pipewire / libspa (pipewire-rs) 0.10 | MIT | audio; links the system libpipewire dynamically |
| ksni 0.3 | Unlicense | panel icon (StatusNotifierItem); Tauri's own tray gets no clicks on Cinnamon |
| notify-rust 4 | MIT / Apache-2.0 | desktop notifications |
| x11rb 0.14 (with XTest) | MIT / Apache-2.0 | charging-case dictation: activate the Claude window, press its shortcut |
| futures-lite, serde, serde_json, log, env_logger | MIT / Apache-2.0 | helpers |
| svelte 5, vite 8, @sveltejs/vite-plugin-svelte | MIT | interface |
| @tauri-apps/api, @tauri-apps/cli | MIT / Apache-2.0 | bridge between interface and Rust |
| typescript, svelte-check | Apache-2.0 / MIT | type checking (development only) |

## Device pictures (AI-generated)

The built-in device pictures show a receiver and a transmitter, recreated with an AI image model so
that no product photo is shipped. Brand logo and model lettering are blurred.

- `app/src/assets/devices/stage-receiver.webp`, `stage-transmitter.webp`: generated on 2026-11-26
  with GPT Image (OpenAI, via the Codex CLI of the ChatGPT app) from the project owner's own photos
  of the device (no DJI product photos or renderings). Prompt core: "studio product photo of the … shown in reference image 1 …
  keep its shape, proportions, camera angle … pure black background". Afterwards the logo on top
  and the "MIC RX" lettering of the receiver and the logo on the transmitter were blurred (Gaussian,
  feathered mask), the black background was made transparent and the pictures were scaled to
  902 × 526 and 573 × 740 px.

Users can replace them with their own pictures in `~/.config/micdeck/devices/` (see README). Such
pictures stay on the user's computer and are not part of this project.

## README animations

`docs/screenshots/ablauf-panel.webp`, `ablauf-seiten.webp`, `ablauf-bild.webp` (the uploaded example is the earlier built-in silhouette) and the screenshots next to them were
recorded from MicDeck's browser preview with demo data. Desktop, panel and panel menu in
`ablauf-panel.webp` are a staged HTML mock-up; the wallpaper was generated by the project owner with
Krea (cropped). The flag and language icons come from the project owner's
[pi-relay](https://github.com/Hendrik4K/pi-relay).

## Panel icons

`app/src-tauri/icons/tray/` (sources in `icons/tray/source/`, 128 px): designed by the project
owner for MicDeck with an AI image model (2026-11-07) – microphone with a fur windscreen swirl in
eight states. No logos, no text. Downscaled to the panel sizes 16–48 px with ImageMagick.

The MicDeck app icon (`app/src-tauri/icons/`, source `icons/source/app-icon.svg`) is the "ready"
icon of the Cinnamon applet (`packaging/cinnamon/micdeck@hendrik4k/icons/micdeck-ok.svg`): a
microphone body with a fur windscreen and level bars, drawn as SVG for MicDeck (2026-11-26). All
sizes made with ImageMagick and `tauri icon`. "DJI" and "DJI Mic Mini" are used only to name
the device; "Claude" only to name the app the optional dictation trigger works with.

## Caveat – handwriting font

- Source: npm package `@fontsource/caveat` (font by Impallari Type), only the Latin subset, weight 600
- Licence: SIL Open Font License 1.1
- Used for: the scribbled "5 Sek. halten zum Einschalten" note on the overview when the receiver is off
