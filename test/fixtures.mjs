// Synthetic metadata containers with known expectations; no private media.
function concat(...parts) {
  const output = new Uint8Array(parts.reduce((size, part) => size + part.length, 0))
  let offset = 0
  for (const part of parts) { output.set(part, offset); offset += part.length }
  return output
}
const ascii = text => Uint8Array.from(text, char => char.charCodeAt(0))
function box(type, payload) {
  const header = new Uint8Array(8)
  new DataView(header.buffer).setUint32(0, payload.length + 8)
  header.set(ascii(type), 4)
  return concat(header, payload)
}

export function createFixtures() {
  const tiff = new Uint8Array(128)
  const view = new DataView(tiff.buffer)
  tiff.set([73, 73, 42, 0, 8, 0, 0, 0])
  view.setUint16(8, 3, true)
  const entry = (offset, tag, type, count, value) => {
    view.setUint16(offset, tag, true)
    view.setUint16(offset + 2, type, true)
    view.setUint32(offset + 4, count, true)
    view.setUint32(offset + 8, value, true)
  }
  entry(10, 0x010f, 2, 8, 60)
  entry(22, 0x0112, 3, 1, 6)
  entry(34, 0x8769, 4, 1, 72)
  tiff.set(ascii('TestCam\0'), 60)
  view.setUint16(72, 1, true)
  entry(74, 0x9003, 2, 20, 100)
  tiff.set(ascii('2026:10:15 12:34:56\0'), 100)
  const exif = concat(ascii('Exif\0\0'), tiff)
  const marker = new Uint8Array([255, 225, 0, 0])
  new DataView(marker.buffer).setUint16(2, exif.length + 2)
  const jpeg = concat(new Uint8Array([255, 216]), marker, exif,
    new Uint8Array([255, 192, 0, 11, 8, 0, 24, 0, 32, 1, 1, 17, 0, 255, 217]))
  const png = new Uint8Array(33)
  png.set([137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82])
  new DataView(png.buffer).setUint32(16, 7)
  new DataView(png.buffer).setUint32(20, 5)
  png.set([8, 2], 24)
  const mvhd = new Uint8Array(100)
  const movieView = new DataView(mvhd.buffer)
  movieView.setUint32(4, 2082844800)
  movieView.setUint32(12, 1000)
  movieView.setUint32(16, 2500)
  const mp4 = concat(box('ftyp', ascii('isom\0\0\0\0isommp42')), box('moov', box('mvhd', mvhd)))
  return [
    { name: '照片😀.data', bytes: jpeg, expected: { kind: 'image', format: 'jpeg', width: 32, height: 24 }, exif: { Make: 'TestCam', DateTimeOriginal: '2026:10:15 12:34:56' } },
    { name: 'renamed-png.data', bytes: png, expected: { kind: 'image', format: 'png', width: 7, height: 5 } },
    { name: 'video.data', bytes: mp4, expected: { kind: 'video', format: 'mp4', duration: 2.5 } },
  ]
}
