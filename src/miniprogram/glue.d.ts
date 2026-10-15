import type { MediaMeta, MiniProgramRuntime } from './types.js'

export function instantiate(runtime: MiniProgramRuntime, path: string): Promise<Record<string, unknown>>;
export function extractMeta(data: Uint8Array, filename: string): MediaMeta;
export function extractMetaFast(data: Uint8Array, filename: string): MediaMeta;
export function extractMetaFastSized(data: Uint8Array, filename: string, sourceSize: number): MediaMeta;
export function extractMetaBatch(items: Array<{ data: Uint8Array; filename: string }>): Array<
  ({ success: true } & MediaMeta) | { success: false; path: string; message: string }
>;
