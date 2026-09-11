import { useState, useRef, useCallback, type DragEvent } from 'react'
import mediaInfoWasmUrl from 'mediainfo.js/MediaInfoModule.wasm?url'
import initMetaprobe, * as metaprobeWasm from '../../wasm/index.js'
import type { MediaMeta } from './types'
import './App.css'

// ── types ──

type MetaprobeResult = MediaMeta

interface ExifrResult {
  [key: string]: unknown
}

interface ComparisonState {
  file: File | null
  isVideo: boolean
  loading: boolean
  metaprobe: { data: MetaprobeResult | null; time: number; error: string | null }
  reference: { data: ExifrResult | null; time: number; error: string | null }
}

// ── metaprobe WASM loader ──

let wasmModule: {
  extractMeta: (data: Uint8Array, filename: string) => MetaprobeResult
  extractMetaFast?: (data: Uint8Array, filename: string) => MetaprobeResult
  extractMetaFastSized?: (data: Uint8Array, filename: string, sourceSize: number) => MetaprobeResult
} | null = null
let wasmLoadAttempted = false

async function loadMetaprobe() {
  if (wasmModule) return wasmModule
  if (wasmLoadAttempted) return null
  wasmLoadAttempted = true
  try {
    await initMetaprobe()
    wasmModule = metaprobeWasm
    return wasmModule
  } catch {
    console.warn('metaprobe WASM module not available. Run `pnpm build:wasm` first.')
    return null
  }
}

// Kick off WASM initialization immediately on module load
const wasmReady = loadMetaprobe()

// ── exifr loader ──

async function runExifr(file: File): Promise<{ data: ExifrResult | null; time: number; error: string | null }> {
  const start = performance.now()
  try {
    const exifr = await import('exifr')
    const data = await exifr.parse(file, { tiff: true, icc: true, gps: true, iptc: true })
    const time = performance.now() - start
    return { data: data ?? {}, time, error: null }
  } catch (e) {
    const time = performance.now() - start
    return { data: null, time, error: String(e) }
  }
}

async function runMediaInfo(file: File): Promise<{ data: ExifrResult | null; time: number; error: string | null }> {
  const start = performance.now()
  try {
    const module = await import('mediainfo.js')
    const mediaInfo = await module.default({ format: 'object', locateFile: () => mediaInfoWasmUrl })
    const buffer = await readAsArrayBuffer(file)
    const bytes = new Uint8Array(buffer)
    try {
      const data = await mediaInfo.analyzeData(
        () => bytes.byteLength,
        async (size: number, offset: number) => bytes.subarray(offset, offset + size),
      )
      return { data: data as ExifrResult, time: performance.now() - start, error: null }
    } finally {
      mediaInfo.close?.()
    }
  } catch (e) {
    return { data: null, time: performance.now() - start, error: String(e) }
  }
}

// ── metaprobe runner ──

async function readAsArrayBuffer(blob: Blob): Promise<ArrayBuffer> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(reader.result as ArrayBuffer)
    reader.onerror = reject
    reader.readAsArrayBuffer(blob)
  })
}

async function sha256Hex(buffer: ArrayBuffer): Promise<string | null> {
  if (!globalThis.crypto?.subtle) return null
  const digest = await globalThis.crypto.subtle.digest('SHA-256', buffer)
  return Array.from(new Uint8Array(digest), byte => byte.toString(16).padStart(2, '0')).join('')
}

function readIsoBoxSize(bytes: Uint8Array, offset: number): number | null {
  if (offset + 8 > bytes.length) return null
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
  const size32 = view.getUint32(offset)
  if (size32 === 0) return bytes.length - offset
  if (size32 !== 1) return size32
  if (offset + 16 > bytes.length) return null
  const high = view.getUint32(offset + 8)
  const low = view.getUint32(offset + 12)
  const size = high * 2 ** 32 + low
  return Number.isSafeInteger(size) ? size : null
}

// Keep only metadata for large containers. The complete file is still used
// for the parallel SHA-256 hash below.
type MetadataPayload = { bytes: Uint8Array; sourceSize?: number; complete: boolean }

function metadataPayload(buffer: ArrayBuffer): MetadataPayload {
  const bytes = new Uint8Array(buffer)
  if (bytes.length >= 4 && bytes[0] === 0xff && bytes[1] === 0xd8) {
    let offset = 2
    while (offset + 3 < bytes.length) {
      while (offset < bytes.length && bytes[offset] !== 0xff) offset += 1
      while (offset < bytes.length && bytes[offset] === 0xff) offset += 1
      if (offset >= bytes.length) break

      const marker = bytes[offset]
      const markerStart = offset - 1
      offset += 1
      if (marker === 0xda) {
        if (offset + 1 < bytes.length) {
          const segmentLength = (bytes[offset] << 8) | bytes[offset + 1]
          return {
            bytes: bytes.subarray(0, Math.min(bytes.length, offset + 2 + segmentLength)),
            complete: true,
          }
        }
        return { bytes: bytes.subarray(0, markerStart + 2), complete: true }
      }
      if (marker === 0xd9 || (marker >= 0xd0 && marker <= 0xd7) || marker === 0x01) continue
      if (offset + 1 >= bytes.length) break
      const segmentLength = (bytes[offset] << 8) | bytes[offset + 1]
      if (segmentLength < 2 || offset + segmentLength > bytes.length) break
      offset += segmentLength
    }
    return { bytes, complete: false }
  }

  // ISO-BMFF metadata is in ftyp/moov boxes. Sending only those boxes keeps
  // large mdat video payloads out of WASM while preserving magic detection.
  if (bytes.length >= 12 && bytes[4] === 0x66 && bytes[5] === 0x74 && bytes[6] === 0x79 && bytes[7] === 0x70) {
    const boxes: Uint8Array[] = []
    let offset = 0
    let foundMoov = false
    while (offset + 8 <= bytes.length) {
      const size = readIsoBoxSize(bytes, offset)
      if (size == null || size < 8 || offset + size > bytes.length) break
      const type = String.fromCharCode(bytes[offset + 4], bytes[offset + 5], bytes[offset + 6], bytes[offset + 7])
      if (type === 'ftyp' || type === 'moov') {
        boxes.push(bytes.subarray(offset, offset + size))
        foundMoov ||= type === 'moov'
      }
      offset += size
      if (foundMoov && offset >= bytes.length) break
    }
    if (foundMoov) {
      const total = boxes.reduce((sum, box) => sum + box.byteLength, 0)
      const compact = new Uint8Array(total)
      let writeOffset = 0
      for (const box of boxes) {
        compact.set(box, writeOffset)
        writeOffset += box.byteLength
      }
      return { bytes: compact, sourceSize: bytes.byteLength, complete: true }
    }
  }

  return { bytes, complete: false }
}

async function readIsoMetadataPayload(file: File): Promise<MetadataPayload> {
  const boxes: Uint8Array[] = []
  let offset = 0
  let foundMoov = false
  while (offset + 8 <= file.size) {
    const header = new Uint8Array(await readAsArrayBuffer(file.slice(offset, Math.min(file.size, offset + 16))))
    const size = readIsoBoxSize(header, 0)
    if (size == null || size < 8 || offset + size > file.size) break
    const type = String.fromCharCode(header[4], header[5], header[6], header[7])
    if (type === 'ftyp' || type === 'moov') {
      boxes.push(new Uint8Array(await readAsArrayBuffer(file.slice(offset, offset + size))))
      foundMoov ||= type === 'moov'
    }
    offset += size
  }
  if (!foundMoov) return metadataPayload(await readAsArrayBuffer(file))
  const total = boxes.reduce((sum, box) => sum + box.byteLength, 0)
  const compact = new Uint8Array(total)
  let writeOffset = 0
  for (const box of boxes) {
    compact.set(box, writeOffset)
    writeOffset += box.byteLength
  }
  return { bytes: compact, sourceSize: file.size, complete: true }
}

async function readMetadataPayload(file: File): Promise<MetadataPayload> {
  const header = new Uint8Array(await readAsArrayBuffer(file.slice(0, 16)))
  const isJpeg = header.length >= 2 && header[0] === 0xff && header[1] === 0xd8
  if (!isJpeg) return metadataPayload(await readAsArrayBuffer(file))

  // JPEG EXIF/ICC data is located before SOS. Read progressively so a large
  // image does not have to be copied into WASM just to find its metadata.
  let limit = Math.min(file.size, 256 * 1024)
  while (true) {
    const payload = metadataPayload(await readAsArrayBuffer(file.slice(0, limit)))
    if (payload.complete || limit >= file.size) return payload
    limit = Math.min(file.size, limit * 2)
  }
}

async function runMetaprobe(file: File, isVideo: boolean): Promise<ComparisonState['metaprobe']> {
  const start = performance.now()
  try {
    const mod = await wasmReady
    if (!mod) {
      return { data: null, time: 0, error: 'WASM module not loaded. Run `pnpm build:wasm` in the project root.' }
    }
    const payload = isVideo
      ? await readIsoMetadataPayload(file)
      : await readMetadataPayload(file)
    const meta = payload.sourceSize != null && mod.extractMetaFastSized
      ? mod.extractMetaFastSized(payload.bytes, file.name, payload.sourceSize)
      : (mod.extractMetaFast ?? mod.extractMeta)(payload.bytes, file.name)
    const time = performance.now() - start
    return { data: meta, time, error: null }
  } catch (e) {
    const time = performance.now() - start
    return { data: null, time, error: String(e) }
  }
}

// ── helpers ──

async function isVideoFile(file: File): Promise<boolean> {
  if (file.type.startsWith('video/')) return true
  const header = new Uint8Array(await readAsArrayBuffer(file.slice(0, 512)))
  const ascii = (offset: number, length: number) => String.fromCharCode(...header.subarray(offset, offset + length))

  if (header.length >= 12 && ascii(0, 4) === 'RIFF' && ascii(8, 4) === 'AVI ') return true
  if (header.length >= 3 && ascii(0, 3) === 'FLV') return true
  if (header.length >= 4 && (ascii(0, 4) === 'OggS' || ascii(0, 4) === '\x00\x00\x01\xba')) return true
  if (header.length >= 4 && header[0] === 0x1a && header[1] === 0x45 && header[2] === 0xdf && header[3] === 0xa3) {
    const probe = new TextDecoder('latin1').decode(header).toLowerCase()
    return probe.includes('webm') || probe.includes('matroska')
  }
  if (header.length >= 377 && header[0] === 0x47 && header[188] === 0x47 && header[376] === 0x47) return true

  if (header.length >= 12 && ascii(4, 4) === 'ftyp') {
    const brands: string[] = []
    for (let offset = 8; offset + 4 <= header.length; offset += 4) brands.push(ascii(offset, 4))
    if (brands.some(brand => ['avif', 'avis', 'heic', 'heix', 'hevc', 'hevx', 'mif1', 'msf1', 'crx ', 'cr3 '].includes(brand))) return false
    return true
  }
  return false
}

function median(values: number[]): number {
  const sorted = [...values].sort((left, right) => left - right)
  return sorted[Math.floor(sorted.length / 2)]
}

async function runMedian<T extends { time: number }>(runner: () => Promise<T>, iterations = 5): Promise<T> {
  const runs: T[] = []
  for (let index = 0; index < iterations; index += 1) runs.push(await runner())
  return { ...runs[runs.length - 1], time: median(runs.map(run => run.time)) }
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`
}

function formatTime(ms: number): string {
  if (ms < 1) return `${(ms * 1000).toFixed(0)} µs`
  if (ms < 1000) return `${ms.toFixed(1)} ms`
  return `${(ms / 1000).toFixed(2)} s`
}

// ── components ──

function DataTable({ data }: { data: Record<string, string | number | unknown> }) {
  const entries = Object.entries(data).sort(([left], [right]) => left.localeCompare(right))
  if (entries.length === 0) return <p style={{ color: 'var(--muted)', fontSize: '0.85rem' }}>No data</p>
  return (
    <table className="data-table">
      <tbody>
        {entries.map(([key, value]) => (
          <tr key={key}>
            <td>{key}</td>
            <td>{String(value)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  )
}

function ResultCard({
  title,
  badge,
  badgeClass,
  time,
  isFaster,
  error,
  children,
}: {
  title: string
  badge: string
  badgeClass: string
  time: number
  isFaster: boolean
  error: string | null
  children: React.ReactNode
}) {
  return (
    <div className="result-card">
      <div className="result-card__header">
        <div className="result-card__title">
          {title}
          <span className={`badge ${badgeClass}`}>{badge}</span>
        </div>
        {time > 0 && (
          <span className={`result-card__time ${isFaster ? 'result-card__time--fast' : 'result-card__time--slow'}`}>
            {formatTime(time)}
            {isFaster && ' ✓'}
          </span>
        )}
      </div>
      <div className="result-card__body">
        {error ? <div className="result-card__error">{error}</div> : children}
      </div>
    </div>
  )
}

function MetaprobeCard({ result }: { result: ComparisonState['metaprobe'] }) {
  const { data } = result
  if (!data) return <p style={{ color: 'var(--muted)', fontSize: '0.85rem' }}>No result</p>

  const basic: Record<string, string | number> = {
    Kind: data.kind,
    Format: data.format,
    Width: `${data.width} px`,
    Height: `${data.height} px`,
    'Color Space': data.colorSpace,
    'File Hash (SHA-256)': data.fileHash ?? 'not requested',
  }

  const technical: Record<string, string | number> = {
    ...(data.duration == null ? {} : { Duration: `${data.duration.toFixed(3)} s` }),
    ...(data.codec == null ? {} : { Codec: data.codec }),
    ...(data.overallBitrate == null ? {} : { 'Overall bitrate': `${data.overallBitrate} bit/s` }),
    ...(data.videoBitrate == null ? {} : { 'Video bitrate': `${data.videoBitrate} bit/s` }),
    ...(data.frameRate == null ? {} : { 'Frame rate': `${data.frameRate.toFixed(3)} fps` }),
    ...(data.creationTime == null ? {} : { 'Creation time': data.creationTime }),
    ...(data.containerCreationTime == null ? {} : { 'Container creation time': data.containerCreationTime }),
  }

  const exifCount = Object.keys(data.exif).length
  const iccCount = Object.keys(data.icc || {}).length
  return (
    <>
      <div className="data-section">
        <div className="data-section__title">Technical Metadata</div>
        <DataTable data={technical} />
      </div>
      <div className="data-section">
        <div className="data-section__title">Basic Info</div>
        <DataTable data={basic} />
      </div>
      <div className="data-section">
        <div className="data-section__title">ICC Profile ({iccCount})</div>
        <DataTable data={data.icc || {}} />
      </div>
      <div className="data-section">
        <div className="data-section__title">EXIF Tags ({exifCount})</div>
        <DataTable data={data.exif} />
      </div>
    </>
  )
}

function ExifrCard({ result }: { result: ComparisonState['reference'] }) {
  const { data } = result
  if (!data) return <p style={{ color: 'var(--muted)', fontSize: '0.85rem' }}>No result</p>

  const entries: Record<string, string> = {}
  for (const [k, v] of Object.entries(data)) {
    if (v === undefined || v === null) continue
    if (typeof v === 'object') {
      entries[k] = JSON.stringify(v)
    } else {
      entries[k] = String(v)
    }
  }

  return (
    <div className="data-section">
      <div className="data-section__title">Parsed Fields ({Object.keys(entries).length})</div>
      <DataTable data={entries} />
    </div>
  )
}

// ── main ──

function App() {
  const [state, setState] = useState<ComparisonState>({
    file: null,
    isVideo: false,
    loading: false,
    metaprobe: { data: null, time: 0, error: null },
    reference: { data: null, time: 0, error: null },
  })

  const processFile = useCallback(async (file: File) => {
    setState(prev => ({ ...prev, file, isVideo: false, loading: true }))
    const video = await isVideoFile(file)
    setState(prev => ({ ...prev, isVideo: video }))

    // Avoid benchmark distortion from both parsers competing for disk/CPU.
    // The CLI benchmark provides warm multi-iteration p50/p95 measurements.
    const metaprobeResult = await runMedian(() => runMetaprobe(file, video))
    const referenceResult = await runMedian(() => video ? runMediaInfo(file) : runExifr(file))
    if (metaprobeResult.data) metaprobeResult.data.fileHash = 'computing…'

    setState(prev => ({
      ...prev,
      loading: false,
      metaprobe: metaprobeResult,
      reference: referenceResult,
    }))

    readAsArrayBuffer(file).then(sha256Hex).then(hash => {
        if (!hash) return
        setState(prev => {
          if (prev.file !== file || !prev.metaprobe.data) return prev
          return {
            ...prev,
            metaprobe: {
              ...prev.metaprobe,
              data: { ...prev.metaprobe.data, fileHash: hash },
            },
          }
        })
      })
  }, [])

  const inputRef = useRef<HTMLInputElement>(null)
  const [dragActive, setDragActive] = useState(false)

  const handleDrop = useCallback((e: DragEvent) => {
    e.preventDefault()
    setDragActive(false)
    const file = e.dataTransfer.files[0]
    if (file) processFile(file)
  }, [processFile])

  const handleChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (file) processFile(file)
  }, [processFile])

  const metaprobeFaster = state.metaprobe.time > 0 && state.reference.time > 0 && state.metaprobe.time <= state.reference.time
  const exifrFaster = state.metaprobe.time > 0 && state.reference.time > 0 && state.reference.time < state.metaprobe.time

  // Count fields
  const metaprobeFieldCount = state.metaprobe.data
    ? 4 + Object.keys(state.metaprobe.data.exif).length + Object.keys(state.metaprobe.data.icc || {}).length
    : 0
  const referenceFieldCount = state.reference.data ? Object.keys(state.reference.data).length : 0

  return (
    <div className="app">
      <header className="header">
        <h1>metaprobe playground</h1>
        <p>
          Drop an image or video to compare <strong>metaprobe</strong>
          <span className="badge badge--wasm">WASM / Rust</span>
          {' '}vs <strong>exifr / mediainfo.js</strong>
          <span className="badge badge--js">Reference</span>
          {' '}· 5-run median
        </p>
      </header>

      {/* Drop zone */}
      <div
        className={`dropzone ${dragActive ? 'dropzone--active' : ''}`}
        onClick={() => inputRef.current?.click()}
        onDragOver={e => { e.preventDefault(); setDragActive(true) }}
        onDragLeave={() => setDragActive(false)}
        onDrop={handleDrop}
      >
        <div className="dropzone__icon">📷</div>
        <div className="dropzone__text">
          {state.loading ? 'Processing...' : 'Drop image or video here or click to select'}
        </div>
        <div className="dropzone__formats">Any image or video format · detected from file contents</div>
        <input
          ref={inputRef}
          type="file"
          className="hidden"
          onChange={handleChange}
        />
      </div>

      {/* File info */}
      {state.file && (
        <div className="file-info">
          <span className="file-info__name">{state.file.name}</span>
          <span className="file-info__detail">{formatSize(state.file.size)}</span>
          <span className="file-info__detail">{state.file.type || 'unknown type'}</span>
        </div>
      )}

      {/* Loading */}
      {state.loading && (
        <div className="loading">
          <div className="spinner" />
          Extracting metadata...
        </div>
      )}

      {/* Results */}
      {!state.loading && state.file && (
        <>
          <div className="results">
            <ResultCard
              title="metaprobe"
              badge="WASM"
              badgeClass="badge--wasm"
              time={state.metaprobe.time}
              isFaster={metaprobeFaster}
              error={state.metaprobe.error}
            >
              <MetaprobeCard result={state.metaprobe} />
            </ResultCard>

            <ResultCard
              title={state.isVideo ? 'mediainfo.js' : 'exifr'}
              badge="Reference"
              badgeClass="badge--js"
              time={state.reference.time}
              isFaster={exifrFaster}
              error={state.reference.error}
            >
              <ExifrCard result={state.reference} />
            </ResultCard>
          </div>

          {/* Summary */}
          <div className="summary">
            <div className="summary__item">
              <div className="summary__label">metaprobe p50</div>
              <div className={`summary__value ${metaprobeFaster ? 'summary__value--success' : ''}`}>
                {state.metaprobe.time > 0 ? formatTime(state.metaprobe.time) : '—'}
              </div>
            </div>
            <div className="summary__item">
              <div className="summary__label">Reference p50</div>
              <div className={`summary__value ${exifrFaster ? 'summary__value--success' : ''}`}>
                {state.reference.time > 0 ? formatTime(state.reference.time) : '—'}
              </div>
            </div>
            <div className="summary__item">
              <div className="summary__label">Speedup</div>
              <div className="summary__value summary__value--accent">
                {state.metaprobe.time > 0 && state.reference.time > 0
                  ? `${(state.reference.time / state.metaprobe.time).toFixed(1)}×`
                  : '—'}
              </div>
            </div>
            <div className="summary__item">
              <div className="summary__label">metaprobe Fields</div>
              <div className="summary__value">{metaprobeFieldCount || '—'}</div>
            </div>
            <div className="summary__item">
              <div className="summary__label">exifr Fields</div>
              <div className="summary__value">{referenceFieldCount || '—'}</div>
            </div>
          </div>
        </>
      )}
    </div>
  )
}

export default App
