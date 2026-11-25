# iceblink

A retained, incremental 2D compositor specification and reference stack for
Rust UI frameworks.

`iceblink` sits below framework paint and above graphics and display APIs.
Layout, styling, text shaping, input dispatch, accessibility, and application
state stay in the UI framework. Frameworks submit atomic `Transaction`s
updating keyed chunks of display items, property trees, paint/parameter slots,
and an order tree; the compositor derives damage, lowers and plans a
renderer-neutral `Frame`, and hands it to a `Renderer` and `Presenter`.

The normative contract lives in [iceblink-spec.md](iceblink-spec.md).

## Seams

Everything above a seam depends only on `iceblink` (which pulls in `bytemuck`,
`kurbo`, `smallvec`, and `web-time`, with no graphics API or async runtime).

| Seam | Trait | Role | Reference implementations |
| --- | --- | --- | --- |
| Scene | `Compositor` | Retains scene state, derives damage, lowers items, plans frames | `iceblink-core` (`precise`), `iceblink_testing::naive` (soundness-only) |
| Draw | `Renderer` | Executes a `Frame` (analytic SDFs, sprites, gradients, clips, groups, filters, paths, shifts, resident chunks) | `iceblink-wgpu` (`iceblink-vello` or `vector_coverage` for paths), `iceblink-cpu` (`vello_cpu` for paths) |
| Output | `Presenter` | Surface/swapchain acquisition, kept-target or buffer-age presentation, refresh pacing | `iceblink-wgpu` (wgpu surface, native Vulkan/DXGI, offscreen, supplied), `iceblink-cpu` (`PixmapPresenter`) |
| Client *(optional)* | `CompositorClient` | Owns the compositor, renderers, latching, and pacing loop | `iceblink-driver` (`InProcess` and `iceblink_driver::thread`) |

Glyph rasterisation (`GlyphRaster`) and font lookup (`FontRegistry`) are also
traits in `iceblink`, with reference implementations in `iceblink-text`.

## Scene model

- **Keyed chunks & Pod items.** Frameworks record flat `#[repr(C)]` Pod items
  (`Quad`, `ShaderQuad`, `Shadow`, `Line`, `GlyphRun`, `Image`, `Marks`,
  `ExternalQuad`, `Backdrop`) plus `PathItem` into chunks keyed by `ChunkKey`.
  Chunks are the unit of retention, replacement, translation (`set_offset`),
  and device-side residency.
- **Property trees & order tree.** Shared state lives in separate
  `TransformNode`, `ScrollNode`, `ClipNode`, and `EffectNode` trees, alongside
  rewritable `Paint` and `ParamsBlock` slots. Paint order is defined
  independently by an `OrderNode` tree. Generational `Id<T>` handles are minted
  client-side via `IdSpace` without compositor round-trips.
- **Incremental work.** `iceblink-core` derives damage across property changes,
  clip writes, and filter/backdrop read extents, skips order subtrees outside
  the damage set, emits opaque covers and pixel shifts for scrolling, and keeps
  lowered chunks resident across frames. Every commit and frame populates
  `Counters` so cost bounds can be asserted directly.
- **Compositor-driven motion.** Optional compositor-tier animations (keyframe
  timing/easing), scroll gesture routing/physics/snapping, and hit testing
  advance frames without framework round-trips.
- **Conformance profiles.** The spec separates *soundness* (correct pixels,
  valid frames, conservative damage) from *precision* (damage tightness, shifts,
  opaque covers, proportional work bounds). `iceblink-testing` provides
  seam-level conformance suites (scene, tier, draw, present, client,
  end-to-end) parameterised by implementation factories.

## Workspace

| Crate | Purpose |
| --- | --- |
| `iceblink` | Contract types, `geom`/`color`/`damage`, seam traits, shader prelude |
| `iceblink-core` | Reference incremental compositor (`plan`, `content`, `motion`) |
| `iceblink-driver` | In-process and threaded (`thread`) frame-lifecycle drivers and pacing |
| `iceblink-wgpu` | Reference wgpu renderer, presenters, `vector` trait, `vector_coverage`, `upload`, `pipeline_cache` |
| `iceblink-vello` | Compute path rasteriser (`VectorRaster`) over Vello |
| `iceblink-wgsl` | WGSL shader modules and application shader assembly |
| `iceblink-cpu` | Reference CPU renderer and pixmap presenter |
| `iceblink-text` | `GlyphRaster` over `swash`/`skrifa` (optional `typeset` feature over `parley`/`fontique`) |
| `iceblink-alloc` | Stable-address `arena` storage and `atlas` placement/eviction |
| `iceblink-testing` | Frame validator, `capture` renderer, `naive` compositor, golden/conformance suites |
| `iceblink-examples` | Windowed `winit` demos (`boxes`, `paragraphs`, `scroll`, `threaded`) |

## Building & running

Requires Rust 1.90+ (edition 2024).

```sh
cargo test --workspace
cargo run -p iceblink-examples --bin scroll
```

## Licence

Licensed under Apache-2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE).
