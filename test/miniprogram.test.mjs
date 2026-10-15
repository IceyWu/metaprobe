import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { test } from 'node:test'
import init, { extractMeta, extractMetaFast, extractMetaFastSized, extractMetaBatch } from '../miniprogram/index.js'
import { Utf8Encoder, Utf8Decoder } from '../miniprogram/utf8.js'
import { createFixtures } from './fixtures.mjs'

test('UTF-8 codecs match platform codecs for Unicode, surrogates and bounded writes', () => {
  const encoder = new Utf8Encoder()
  const decoder = new Utf8Decoder('utf-8', { ignoreBOM: true, fatal: true })
  for (const text of ['', 'ASCII', '照片😀é\uFEFF', '\ud800x\udfff', '\u0000\u07ff\u0800\uffff']) {
    assert.deepEqual(encoder.encode(text), new TextEncoder().encode(text))
    assert.equal(decoder.decode(encoder.encode(text)), new TextDecoder('utf-8', { ignoreBOM: true, fatal: true }).decode(encoder.encode(text)))
    for (let size = 0; size <= text.length * 4; size++) {
      const actual = new Uint8Array(size)
      const expected = new Uint8Array(size)
      assert.deepEqual(encoder.encodeInto(text, actual), new TextEncoder().encodeInto(text, expected))
      assert.deepEqual(actual, expected)
    }
  }
  for (const invalid of [[0xc0, 0x80], [0xed, 0xa0, 0x80], [0xf4, 0x90, 0x80, 0x80], [0xe2, 0x82], [0xff]]) {
    assert.throws(() => decoder.decode(new Uint8Array(invalid)), TypeError)
  }
})

test('invalid config and failed initialization can be retried', async () => {
  await assert.rejects(init({ wasmPath: 'test.wasm' }), /WXWebAssembly/)
  const broken = { instantiate: async () => { throw new Error('load failed') } }
  await assert.rejects(init({ runtime: broken }), /wasmPath/)
  await assert.rejects(init({ runtime: broken, wasmPath: 'test.wasm' }), /load failed/)
})

test('real WASM path loader: concurrent init, image/video/EXIF/hash/batch', async () => {
  const bytes = await readFile(new URL('../miniprogram/index_bg.wasm', import.meta.url))
  let calls = 0
  const runtime = { async instantiate(path, imports) {
    assert.equal(path, 'static/metaprobe/index_bg.wasm')
    calls++
    return (await WebAssembly.instantiate(bytes, imports)).instance
  } }
  const options = { wasmPath: 'static/metaprobe/index_bg.wasm', runtime }
  await Promise.all([init(options), init(options)])
  assert.equal(calls, 1)
  await assert.rejects(init({ ...options, wasmPath: 'other.wasm' }), /different/)
  for (const fixture of createFixtures()) {
    const fast = extractMetaFastSized(fixture.bytes, fixture.name, fixture.bytes.length)
    for (const [key, value] of Object.entries(fixture.expected)) assert.equal(fast[key], value, `${fixture.name}: ${key}`)
    for (const [key, value] of Object.entries(fixture.exif ?? {})) assert.ok(fast.exif[key]?.includes(value), `${key}: ${fast.exif[key]}`)
    assert.equal(extractMetaFast(fixture.bytes, fixture.name).fileHash, undefined)
    assert.equal(extractMeta(fixture.bytes, fixture.name).fileHash, createHash('sha256').update(fixture.bytes).digest('hex'))
    if (fast.kind === 'video') assert.equal(extractMetaFastSized(fixture.bytes, fixture.name, 1000000).overallBitrate, 3200000)
  }
  const batch = extractMetaBatch([
    ...createFixtures().map(fixture => ({ data: fixture.bytes, filename: fixture.name })),
    { data: new Uint8Array([1, 2, 3]), filename: '损坏.data' },
  ])
  assert.deepEqual(batch.map(item => item.success), [true, true, true, false])
  assert.equal(batch[3].path, '损坏.data')
  assert.throws(() => extractMetaFast(new Uint8Array([1, 2, 3]), 'broken.data'))
  const glue = await readFile(new URL('../miniprogram/glue.js', import.meta.url), 'utf8')
  assert.doesNotMatch(glue, /\b(?:fetch|WebAssembly|document|window)\b|import\.meta/)
})

test('browser entry still initializes and returns the same metadata', async () => {
  const browser = await import('../wasm/index.js')
  const wasm = await readFile(new URL('../wasm/index_bg.wasm', import.meta.url))
  browser.initSync({ module: new WebAssembly.Module(wasm) })
  for (const fixture of createFixtures()) {
    assert.deepEqual(
      browser.extractMetaFastSized(fixture.bytes, fixture.name, fixture.bytes.length),
      extractMetaFastSized(fixture.bytes, fixture.name, fixture.bytes.length),
    )
  }
})
