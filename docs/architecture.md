# DeckCraft architecture

> **Last reviewed:** 2026-10-10 · **Last updated:** 2026-10-13 · **Change:** minor (outside callers go through `Session::call`; previous: major, created from the code on main at d556ff9; AGENTS.md §4 keeps the rules) · **Target:** Microsoft PowerPoint for Mac 16.113 (Microsoft 365)

How DeckCraft is built today. The binding rules (layering, everything is a command, thin UI, never
break wasm, never crash) are in [AGENTS.md](../AGENTS.md) §3–§5; this document describes what
exists. 117k lines of Rust in 23 crates, 3 apps and `xtask`; 504 `#[test]` functions
(`cargo xtask stats` counts them per crate).

## Layers

`cargo xtask layers` enforces that a crate depends only on lower layers (the table in
`xtask/src/layers.rs`, which is authoritative; AGENTS.md §4 gives the simplified picture), and that
nothing below L6 uses egui, eframe, winit or rfd.

| Layer | Crates | What they do |
|---|---|---|
| L0 | `geom`, `color`; standalone `bitstream`, `matroska`, `ogg`, `opus` | EMU/pt units, transforms, ~150 preset shape generators re-derived from ECMA-376, adjust handles, hit testing; RGB/HSL and DrawingML colour transforms; MKV/WebM and Ogg demuxers, the Opus decoder |
| L1 | `model`, `fonts`; `h264`, `hevc`, `vp9`, `av1`, `isobmff` | Presentation, masters, layouts, slides, shapes, text, themes, tables, charts, transitions, animations, placeholder inheritance (`resolve`); font database (craft-fonts + system) with metric-compatible substitutes for Office fonts (Carlito for Calibri, Caladea for Cambria, Liberation for Arial/Times); video decoders and the MP4/MOV demuxer |
| L2 | `text`, `anim`, `format`, `media` | Shaping (HarfRust), UAX #9 bidi, UAX #14 line breaking, autofit, caret hit testing; per-shape animation state over time, the show state machine, transition frame layouts, Morph matching; `.deckcraft` (zip + JSON + media) and outline text; media probing, decoding, mixer and clock |
| L3 | `render`, `pptx` | `vello_cpu` raster of slides, masters and layouts (fills, outlines, effects, pictures, tables, charts, Morph frames); PPTX import/export from ECMA-376 |
| L4 | `pdf` | PDF export through krilla |
| L5 | `engine` | Sessions, documents, selection, undo history on shared slide snapshots, the command registry (226 commands in `cmd/*`), pointer tools, keys, AutoRecover, the panic guard (`guard.rs`) |
| L6 | `ui-egui`, `mcp` | The PowerPoint-style interface (ribbon with 10 tabs + contextual tabs, canvas, thumbnails, panes, 27 dialogs, show and presenter windows, the JSON control channel); the MCP server (JSON-RPC 2.0 over stdio), headless or connected to a running app |
| apps | `deckcraft` (desktop: eframe/wgpu, cpal audio, logging), `deckcraft-cli` (run, render, convert, mcp, app), `deckcraft-web` (trunk; WebGPU with WebGL2 fallback), `xtask` | Exempt from layering |

The media codecs and containers are clean-room pure Rust, 59k lines, shared in spirit with
FilmCraft.

## Data model

A `Presentation` holds a theme set, slide masters with their layouts, slides, sections, custom
shows, comments, media and document properties. Shapes are a tree (`ShapeKind`: preset or custom
geometry, picture, table, chart, group, connector, ink, media, SmartArt-as-group, and `Opaque` for
anything read from PPTX that DeckCraft doesn't model: its XML is kept and written back). Text
bodies hold paragraphs of runs with DrawingML properties; formatting resolves slide → layout →
master → theme at render time. Transitions live on slides; animations are the flat ordered list the
Animation Pane shows, rebuilt into PowerPoint's timing tree on save.

## Commands and frontends

Every user-visible action is a `CommandSpec` (id, label, ribbon/menu place, shortcut, params doc,
`enabled`, `run`) in `crates/engine/src/cmd/*`. The UI, `deckcraft-cli run`, the JSON control
channel (`deckcraft --control PORT`, [control-protocol.md](control-protocol.md)) and MCP
([mcp.md](mcp.md)) all run the same commands, so they behave identically. The UI calls
`Session::execute`; the CLI, control channel and MCP call `Session::call`, which also refuses
parameters a command doesn't take and answers in words what a command changed.
Programmatic calls never open dialogs. Pointer gestures go through `Session::pointer`, keys through
`Session::key`.

## Rendering and the show

The renderer draws a slide to an RGBA pixmap on the CPU; the UI uploads it as an egui texture.
Transitions are layers of textured quads over the old and new slide images (fake 3-D from
trapezoids). The show window and the presenter view are egui viewports; animation state per shape
comes from `anim::timeline` each frame. PDF export renders each slide at print resolution and
overlays an invisible real-text layer with embedded font subsets.

## File I/O

- **PPTX** (`crates/pptx`): lenient reader (unknown parts skipped, malformed parts logged and
  ignored, sizes and depths bounded, zip bombs capped), writer that PowerPoint opens without
  repair. Unknown elements round-trip through `Opaque` and `raw_ext` fields.
- **Native** (`crates/format`): zip with `mimetype`, `presentation.json` and `media/<id>`; readers
  ignore unknown fields; `.slidecraft` files from before the rename still open.
- Formats and their gaps: [file-format-parity.md](file-format-parity.md).

## Quality gates

`cargo xtask ci`: rustfmt check, clippy `-D warnings`, tests, layering, asset attribution
(`cargo xtask assets`), wasm32 check. CI runs it on Ubuntu for every PR and push to `main`.
This fork releases on a `v*` tag: one Linux x86_64 tarball in upstream's shape, with
`SHA256SUMS.txt`, as a GitHub release of the fork. Upstream's signed macOS, Windows, FreeBSD and
web builds are not part of it.

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-13 | minor | The fork's release workflow: a Linux x86_64 tarball on tag |
| 2026-10-13 | minor | The CLI, control channel and MCP go through `Session::call` |
| 2026-10-10 | major | Created from the code on main (d556ff9) and AGENTS.md §4 |
