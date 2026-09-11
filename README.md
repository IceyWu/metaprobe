<p align="center">
  <img src="./assets/metaprobe-logo.png" width="156" alt="metaprobe logo" />
</p>

<h1 align="center">metaprobe</h1>

<p align="center">
  Rust-powered metadata extraction for images and videos.<br />
  One core parser, available through Node.js native bindings and browser WASM.
</p>

<p align="center">
  <a href="./README.zh-CN.md">简体中文</a> · English
</p>

<p align="center">
  <img src="https://img.shields.io/badge/license-MIT-7c3aed" alt="MIT license" />
  <img src="https://img.shields.io/badge/core-Rust-f97316" alt="Rust core" />
  <img src="https://img.shields.io/badge/runtime-Node.js%20%7C%20Browser-06b6d4" alt="Node.js and browser" />
  <img src="https://img.shields.io/badge/formats-content--detected%20image%20%26%20video%20containers-2563eb" alt="Content-detected image and video containers" />
  <img src="https://img.shields.io/badge/npm-metaprobe-CB3837?logo=npm&logoColor=white" alt="npm package metaprobe" />
  <img src="https://img.shields.io/badge/npm%20version-0.1.0-CB3837?logo=npm&logoColor=white" alt="npm package version 0.1.0" />
  <img src="https://img.shields.io/badge/npm%20status-not%20published-lightgrey?logo=npm&logoColor=white" alt="npm not published" />
</p>

`metaprobe` parses image and video metadata without spawning `ffprobe`,
MediaInfo, ExifTool, or any other external binary. The parsing logic lives in
one Rust core and is exposed through a Node.js N-API adapter and a browser
WASM adapter.

## Features

- Content-first format detection; file extensions are only a fallback hint.
- Common image formats including JPEG, PNG, GIF, BMP, TIFF/RAW, WebP, ICO,
  SVG, JPEG 2000, JPEG XL, HEIC, AVIF, CR3, and more.
- Common video containers including MOV/MP4, AVI, WebM/MKV, FLV, MPEG-TS,
  MPEG program streams, Ogg, and more.
- Recognized containers do not need a known filename suffix; unsupported
  details remain safe container-level results instead of extension errors.
- EXIF, GPS, ICC profile, color space, dimensions, and SHA-256 for images.
- QuickTime/ISO-BMFF metadata, capture time, container time, codec, duration,
  frame rate, and bitrate for videos.
- Native Node.js API for filesystem paths and parallel batch extraction.
- WASM API for `Uint8Array` input, fast extraction, optional hashing, and batch
  extraction in browsers.
- No runtime JavaScript parsing dependency for the core metadata path.

## Installation

After the first npm release:

```bash
pnpm add metaprobe
```

The published Node.js package includes the platform native binding and the
browser WASM build. During local development, build the binding and WASM
artifacts first:

```bash
pnpm install
pnpm build
```

## Node.js API

```ts
import { extractMeta, extractMetaBatch } from 'metaprobe'

const meta = await extractMeta('./IMG_6920.MOV', { hash: false })

console.log(meta.kind)                  // "video"
console.log(meta.width, meta.height)
console.log(meta.duration, meta.frameRate)
console.log(meta.exif.GPSLatitude, meta.exif.GPSLongitude)
console.log(meta.creationTime)          // camera-recorded time
console.log(meta.containerCreationTime) // container creation time

const results = await extractMetaBatch(
  ['./photo.jpg', './video.mov'],
  { concurrency: 4, hash: false },
)
```

`hash` defaults to enabled in the native API. Disable it for the lowest
latency when a content hash is not needed.

## Browser / WASM API

```ts
import init, { extractMetaFastSized } from 'metaprobe'

await init()

const bytes = new Uint8Array(await file.arrayBuffer())
const meta = extractMetaFastSized(bytes, file.name, file.size)
```

Use `extractMeta` when the WASM implementation should calculate SHA-256. Use
`extractMetaFast` when hashing is handled separately by the Web Crypto API.
`extractMetaFastSized` is useful for video slices because it preserves the
original file size for bitrate calculation.

## Returned data

Images expose fields such as:

```ts
meta.exif.Make
meta.exif.Model
meta.exif.DateTimeOriginal
meta.exif.GPSLatitude
meta.exif.GPSLongitude
meta.exif.GPSAltitude
meta.icc.ProfileDescription
meta.colorSpace
```

Videos additionally expose:

```ts
meta.duration
meta.codec
meta.frameRate
meta.overallBitrate
meta.videoBitrate
meta.creationTime
meta.containerCreationTime
meta.metadata // QuickTime metadata such as location/device fields
```

## Architecture

```text
Node.js consumer  -> native/index.js -> crates/napi -> crates/core
Browser consumer  -> wasm/index.js   -> crates/wasm -> crates/core
                                           \-> shared Rust parsing logic
Playground        -> React/Vite comparison UI
                   -> metaprobe WASM vs exifr / mediainfo.js references
```

The core is deliberately independent of Node.js and browser APIs. Adapters
only handle I/O, JavaScript value conversion, and platform-specific scheduling.
This keeps the parsing behavior aligned across native and browser consumers.

## Performance

Performance depends on file size, metadata complexity, runtime, hardware, and
whether hashing is enabled. This repository does not include benchmark media,
so no fixed speed numbers are claimed here. Run the benchmark with your own
representative files before making release or comparison claims:

```bash
pnpm benchmark -- ./photo.jpg ./video.mov
```

## Playground

The playground compares the WASM build with `exifr` for images and
`mediainfo.js` for videos:

```bash
pnpm build:wasm
pnpm --dir playground dev
```

The reference parsers are development-only dependencies and are not runtime
dependencies of the published `metaprobe` package.

## Development and checks

```bash
pnpm build:napi
pnpm build:wasm
pnpm --dir playground lint
pnpm --dir playground build

cargo test -p metaprobe-core
cargo clippy --workspace -- -D warnings
```

The CI matrix builds native bindings for Windows, Linux, and macOS targets,
and separately validates the WASM and playground builds.

## Release

This project uses Changesets for versioning and npm releases. The first public
release is prepared as `0.1.0`; later changes should add a changeset:

```bash
pnpm changeset
pnpm version-packages
pnpm release
```

On GitHub, the release workflow builds the native bindings for the supported
platforms, assembles the WASM package, and then creates a version PR or
publishes through Changesets. Configure the repository `NPM_TOKEN` secret
before enabling the first publish.

## License

MIT
