import { readFile } from 'node:fs/promises'
import { performance } from 'node:perf_hooks'
import process from 'node:process'

const iterations = Number(process.env.METAPROBE_BENCH_ITERATIONS || 20)
const paths = process.argv.slice(2)

if (paths.length === 0) {
  console.error('Usage: pnpm benchmark -- <image-or-video> [...files]')
  process.exitCode = 1
  process.exit()
}

const metaprobe = await import('../native/index.js')
const exifrModule = await import('exifr').catch(() => null)
const exifr = exifrModule?.default ?? exifrModule
const mediainfoModule = await import('mediainfo.js').catch(() => null)

function percentile(values, ratio) {
  const sorted = [...values].sort((a, b) => a - b)
  return sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * ratio))]
}

async function timed(label, fn) {
  const samples = []
  let result
  for (let index = 0; index < iterations + 1; index += 1) {
    const start = performance.now()
    result = await fn()
    if (index > 0) samples.push(performance.now() - start)
  }
  return {
    label,
    result,
    p50: percentile(samples, 0.5),
    p95: percentile(samples, 0.95),
    average: samples.reduce((sum, value) => sum + value, 0) / samples.length,
  }
}

async function runExifr(filePath) {
  if (!exifr?.parse) return null
  const data = await readFile(filePath)
  return exifr.parse(data, { tiff: true, icc: true, gps: true, iptc: true, xmp: true })
}

async function createMediaInfoRunner() {
  if (!mediainfoModule) return null
  const factory = mediainfoModule.default ?? mediainfoModule
  const mediaInfo = await factory({ format: 'object' })
  return {
    analyze(data) {
      return mediaInfo.analyzeData(
        () => data.byteLength,
        async (size, offset) => data.subarray(offset, offset + size),
      )
    },
    close() {
      mediaInfo.close?.()
    },
  }
}

function isVideoBytes(data) {
  if (data.subarray(0, 4).toString('ascii') === 'RIFF' && data.subarray(8, 12).toString('ascii') === 'AVI ') return true
  if (data.subarray(0, 3).toString('ascii') === 'FLV' || data.subarray(0, 4).toString('ascii') === 'OggS') return true
  if (data.subarray(0, 4).equals(Buffer.from([0x1a, 0x45, 0xdf, 0xa3]))) {
    const probe = data.subarray(0, 4096).toString('latin1').toLowerCase()
    return probe.includes('webm') || probe.includes('matroska')
  }
  if (data.length >= 377 && data[0] === 0x47 && data[188] === 0x47 && data[376] === 0x47) return true
  if (data.length < 12 || data.toString('ascii', 4, 8) !== 'ftyp') return false
  const size = Math.min(data.length, data.readUInt32BE(0) || data.length)
  const imageBrands = new Set(['avif', 'avis', 'heic', 'heix', 'hevc', 'hevx', 'mif1', 'msf1', 'crx ', 'cr3 '])
  for (let offset = 8; offset + 4 <= size; offset += 4) {
    if (imageBrands.has(data.toString('ascii', offset, offset + 4))) return false
  }
  return true
}

for (const filePath of paths) {
  const data = await readFile(filePath)
  const isVideo = isVideoBytes(data)
  const nativeFast = await timed('metaprobe (no hash)', () =>
    metaprobe.extractMeta(filePath, { hash: false }),
  )
  const native = await timed('metaprobe (with hash)', () => metaprobe.extractMeta(filePath))
  const mediaInfo = isVideo ? await createMediaInfoRunner() : null
  const reference = isVideo
    ? mediaInfo && await timed('mediainfo.js', async () => mediaInfo.analyze(await readFile(filePath)))
    : await timed('exifr', () => runExifr(filePath))
  const referenceName = isVideo ? 'mediainfo.js' : 'exifr'

  console.log(`\n${filePath} (${data.byteLength} bytes)`)
  console.table([
    { parser: native.label, p50_ms: native.p50.toFixed(3), p95_ms: native.p95.toFixed(3), avg_ms: native.average.toFixed(3) },
    { parser: nativeFast.label, p50_ms: nativeFast.p50.toFixed(3), p95_ms: nativeFast.p95.toFixed(3), avg_ms: nativeFast.average.toFixed(3) },
    reference
      ? { parser: reference.label, p50_ms: reference.p50.toFixed(3), p95_ms: reference.p95.toFixed(3), avg_ms: reference.average.toFixed(3) }
      : { parser: referenceName, p50_ms: 'not installed', p95_ms: '-', avg_ms: '-' },
  ])
  if (reference) {
    console.log(`p50 speedup (${referenceName} / metaprobe no hash): ${(reference.p50 / nativeFast.p50).toFixed(2)}x`)
  }
  console.log('metaprobe result:', JSON.stringify({
    kind: native.result.kind,
    format: native.result.format,
    width: native.result.width,
    height: native.result.height,
    duration: native.result.duration,
    codec: native.result.codec,
    frameRate: native.result.frameRate,
    exifKeys: Object.keys(native.result.exif ?? {}).length,
    metadataKeys: Object.keys(native.result.metadata ?? {}).length,
  }))
  mediaInfo?.close()
}
