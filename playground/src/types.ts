export interface MediaMeta {
  kind: 'image' | 'video'
  format: 'jpeg' | 'png' | 'webp' | 'heic' | 'avif' | 'mov' | 'mp4'
  width: number
  height: number
  colorSpace: string
  exif: Record<string, string>
  icc: Record<string, string>
  duration?: number
  creationTime?: string
  containerCreationTime?: string
  codec?: string
  overallBitrate?: number
  videoBitrate?: number
  frameRate?: number
  metadata: Record<string, string>
  fileHash?: string
}
