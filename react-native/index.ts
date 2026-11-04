import { requireNativeModule } from "expo-modules-core";

export type MetaprobeMediaMeta = {
  kind: "image" | "video";
  format: string;
  width: number;
  height: number;
  colorSpace: string;
  exif: Record<string, string>;
  icc: Record<string, string>;
  duration?: number;
  creationTime?: string;
  containerCreationTime?: string;
  codec?: string;
  overallBitrate?: number;
  videoBitrate?: number;
  frameRate?: number;
  metadata: Record<string, string>;
  fileHash?: string;
};
export type ImagePixels = {
  width: number;
  height: number;
  rgbaBase64: string;
};

type NativeMetaprobeModule = {
  extractMeta(uri: string, filename: string): Promise<MetaprobeMediaMeta | string>;
  getImagePixels(uri: string, maxDimension: number): Promise<ImagePixels>;
};

const NativeMetaprobe = requireNativeModule<NativeMetaprobeModule>("Metaprobe");

export async function extractMeta(
  uri: string,
  filename: string
): Promise<MetaprobeMediaMeta> {
  const nativeResult = await NativeMetaprobe.extractMeta(uri, filename);
  const metadata =
    typeof nativeResult === "string" ? JSON.parse(nativeResult) : nativeResult;
  if ("error" in metadata) {
    throw new Error(String(metadata.error));
  }
  return metadata as MetaprobeMediaMeta;
}

export function getImagePixels(uri: string, maxDimension = 100): Promise<ImagePixels> {
  return NativeMetaprobe.getImagePixels(uri, maxDimension);
}
