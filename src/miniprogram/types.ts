export interface MediaMeta {
  kind: string;
  format: string;
  width: number;
  height: number;
  colorSpace: string;
  exif: Record<string, string>;
  icc: Record<string, string>;
  metadata: Record<string, string>;
  duration?: number;
  codec?: string;
  frameRate?: number;
  overallBitrate?: number;
  videoBitrate?: number;
  creationTime?: string;
  containerCreationTime?: string;
  fileHash?: string;
}

export interface MiniProgramRuntime {
  instantiate(path: string, imports: Record<string, Record<string, unknown>>): Promise<
    { exports: Record<string, unknown> } |
    { instance: { exports: Record<string, unknown> }; module?: unknown }
  >;
}

export interface InitOptions {
  /** Copy miniprogram/index_bg.wasm into your app, e.g. static/metaprobe/index_bg.wasm. */
  wasmPath: string;
  /** Defaults to the mini-program's WXWebAssembly global. */
  runtime?: MiniProgramRuntime;
}
