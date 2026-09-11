# metaprobe playground

Local browser comparison for the metaprobe WASM build against `exifr` for
images and `mediainfo.js` for videos.

```bash
pnpm --dir .. build:wasm
pnpm dev
```

The cards show one end-to-end interactive run. Use the repository-level
`pnpm benchmark -- <files...>` command for warmed p50/p95 measurements.
