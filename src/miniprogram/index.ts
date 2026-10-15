import { instantiate } from './glue.js'
import type { InitOptions, MiniProgramRuntime } from './types.js'

export { extractMeta, extractMetaFast, extractMetaFastSized, extractMetaBatch } from './glue.js'
export type { InitOptions, MediaMeta, MiniProgramRuntime } from './types.js'
export { Utf8Encoder, Utf8Decoder } from './utf8.js'

declare const WXWebAssembly: MiniProgramRuntime | undefined

let initialization: Promise<Record<string, unknown>> | undefined
let initialPath: string | undefined
let initialRuntime: MiniProgramRuntime | undefined

/** Load a packaged WASM file without fetch or browser globals. Failed loads may be retried. */
export default function init(options: InitOptions): Promise<Record<string, unknown>> {
  const runtime = options.runtime ?? (typeof WXWebAssembly === 'undefined' ? undefined : WXWebAssembly)
  const path = options.wasmPath
  if (!runtime || typeof runtime.instantiate !== 'function') {
    return Promise.reject(new Error('WXWebAssembly is unavailable; use this entry in a WeChat mini-program.'))
  }
  if (typeof path !== 'string' || !path.trim()) {
    return Promise.reject(new TypeError('wasmPath must be the packaged mini-program WASM file path.'))
  }
  if (initialization) {
    if (path !== initialPath || runtime !== initialRuntime) {
      return Promise.reject(new Error('metaprobe was already initialized with a different WASM path or runtime.'))
    }
    return initialization
  }
  initialPath = path
  initialRuntime = runtime
  initialization = instantiate(runtime, path).catch(error => {
    initialization = undefined
    initialPath = undefined
    initialRuntime = undefined
    throw error
  })
  return initialization
}
