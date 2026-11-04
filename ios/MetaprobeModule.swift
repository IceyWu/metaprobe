import ExpoModulesCore
import CoreGraphics
import Foundation
import MetaprobeFFI
import UIKit

public final class MetaprobeModule: Module {
  private func decodePixels(uri: String, maxDimension: Int) throws -> [String: Any] {
    guard let url = URL(string: uri), url.isFileURL else {
      throw NSError(
        domain: "Metaprobe",
        code: 3,
        userInfo: [NSLocalizedDescriptionKey: "Expected a local image file URI"]
      )
    }
    let data = try Data(contentsOf: url, options: [.mappedIfSafe])
    guard let image = UIImage(data: data) else {
      throw NSError(
        domain: "Metaprobe",
        code: 4,
        userInfo: [NSLocalizedDescriptionKey: "Cannot decode selected image"]
      )
    }

    let limit = max(1, maxDimension)
    let scale = min(1.0, Double(limit) / Double(max(image.size.width, image.size.height)))
    let width = max(1, Int((Double(image.size.width) * scale).rounded()))
    let height = max(1, Int((Double(image.size.height) * scale).rounded()))
    let renderSize = CGSize(width: width, height: height)
    UIGraphicsBeginImageContextWithOptions(renderSize, false, 1)
    image.draw(in: CGRect(origin: .zero, size: renderSize))
    let renderedImage = UIGraphicsGetImageFromCurrentImageContext()
    UIGraphicsEndImageContext()
    guard let source = renderedImage?.cgImage else {
      throw NSError(
        domain: "Metaprobe",
        code: 5,
        userInfo: [NSLocalizedDescriptionKey: "Cannot render selected image"]
      )
    }

    var rgba = [UInt8](repeating: 0, count: width * height * 4)
    let colorSpace = CGColorSpaceCreateDeviceRGB()
    let bitmapInfo = CGBitmapInfo.byteOrder32Big.rawValue | CGImageAlphaInfo.premultipliedLast.rawValue

    let rendered = rgba.withUnsafeMutableBytes { buffer -> Bool in
      guard let context = CGContext(
        data: buffer.baseAddress,
        width: width,
        height: height,
        bitsPerComponent: 8,
        bytesPerRow: width * 4,
        space: colorSpace,
        bitmapInfo: bitmapInfo
      ) else {
        return false
      }
      context.interpolationQuality = .high
      context.draw(source, in: CGRect(x: 0, y: 0, width: width, height: height))
      return true
    }
    guard rendered else {
      throw NSError(
        domain: "Metaprobe",
        code: 6,
        userInfo: [NSLocalizedDescriptionKey: "Cannot access decoded image pixels"]
      )
    }

    return [
      "width": width,
      "height": height,
      "rgbaBase64": Data(rgba).base64EncodedString()
    ]
  }

  public func definition() -> ModuleDefinition {
    Name("Metaprobe")

    AsyncFunction("extractMeta") { (uri: String, filename: String) throws -> [String: Any] in
      guard let url = URL(string: uri), url.isFileURL else {
        throw NSError(
          domain: "Metaprobe",
          code: 1,
          userInfo: [NSLocalizedDescriptionKey: "Expected a local file URI"]
        )
      }
      let data = try Data(contentsOf: url, options: [.mappedIfSafe])
      let metadata = try Metaprobe.parse(data: data, filename: filename)
      let encoded = try JSONEncoder().encode(metadata)
      guard let result = try JSONSerialization.jsonObject(with: encoded) as? [String: Any] else {
        throw NSError(
          domain: "Metaprobe",
          code: 2,
          userInfo: [NSLocalizedDescriptionKey: "Invalid metadata result"]
        )
      }
      return result
    }

    AsyncFunction("getImagePixels") { (uri: String, maxDimension: Int) throws -> [String: Any] in
      try self.decodePixels(uri: uri, maxDimension: maxDimension)
    }
  }
}
