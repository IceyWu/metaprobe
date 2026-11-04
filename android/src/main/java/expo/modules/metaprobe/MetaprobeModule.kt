package expo.modules.metaprobe

import android.net.Uri
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.ImageDecoder
import android.os.Build
import android.util.Base64
import expo.modules.kotlin.functions.Coroutine
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition
import java.io.FileNotFoundException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlin.math.roundToInt

class MetaprobeModule : Module() {
  companion object {
    init {
      System.loadLibrary("metaprobe_android")
    }
  }

  private external fun nativeExtract(bytes: ByteArray, filename: String): String

  private fun decodeBitmap(uri: Uri, maxDimension: Int, context: android.content.Context): Bitmap {
    val dimension = maxDimension.coerceAtLeast(1)
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
      runCatching {
        val source = ImageDecoder.createSource(context.contentResolver, uri)
        ImageDecoder.decodeBitmap(source) { decoder, info, _ ->
          decoder.allocator = ImageDecoder.ALLOCATOR_SOFTWARE
          val longestSide = maxOf(info.size.width, info.size.height)
          val sampleSize = maxOf(1, (longestSide + dimension - 1) / dimension)
          decoder.setTargetSampleSize(sampleSize)
        }
      }.getOrNull()?.let { return it }
    }
    val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
    context.contentResolver.openInputStream(uri)?.use {
      BitmapFactory.decodeStream(it, null, bounds)
    }
    var sampleSize = 1
    while (maxOf(bounds.outWidth, bounds.outHeight) / sampleSize > dimension * 2) {
      sampleSize *= 2
    }
    val options = BitmapFactory.Options().apply { inSampleSize = sampleSize }
    return context.contentResolver.openInputStream(uri)?.use {
      BitmapFactory.decodeStream(it, null, options)
    }
      ?: throw FileNotFoundException("Cannot decode selected image: $uri")
  }

  private fun decodePixels(uri: Uri, maxDimension: Int, context: android.content.Context): Map<String, Any> {
    val decoded = decodeBitmap(uri, maxDimension, context)
    val scale = minOf(1.0, maxDimension.coerceAtLeast(1).toDouble() / maxOf(decoded.width, decoded.height))
    val width = maxOf(1, (decoded.width * scale).roundToInt())
    val height = maxOf(1, (decoded.height * scale).roundToInt())
    val bitmap = if (width == decoded.width && height == decoded.height) decoded
    else Bitmap.createScaledBitmap(decoded, width, height, true)

    try {
      val argb = IntArray(bitmap.width * bitmap.height)
      bitmap.getPixels(argb, 0, bitmap.width, 0, 0, bitmap.width, bitmap.height)
      val rgba = ByteArray(argb.size * 4)
      for (index in argb.indices) {
        val pixel = argb[index]
        val offset = index * 4
        rgba[offset] = ((pixel shr 16) and 0xff).toByte()
        rgba[offset + 1] = ((pixel shr 8) and 0xff).toByte()
        rgba[offset + 2] = (pixel and 0xff).toByte()
        rgba[offset + 3] = ((pixel ushr 24) and 0xff).toByte()
      }
      return mapOf(
        "width" to bitmap.width,
        "height" to bitmap.height,
        "rgbaBase64" to Base64.encodeToString(rgba, Base64.NO_WRAP)
      )
    } finally {
      if (bitmap !== decoded) bitmap.recycle()
      decoded.recycle()
    }
  }

  override fun definition() = ModuleDefinition {
    Name("Metaprobe")

    AsyncFunction("extractMeta") Coroutine { uri: String, filename: String ->
      val context = appContext.reactContext
        ?: throw IllegalStateException("React context is unavailable")
      val mediaUri = Uri.parse(uri)
      val bytes = withContext(Dispatchers.IO) {
        context.contentResolver.openInputStream(mediaUri)?.use { it.readBytes() }
          ?: throw FileNotFoundException("Cannot read selected media: $uri")
      }
      val resolvedName = filename.ifBlank { mediaUri.lastPathSegment ?: "media" }
      nativeExtract(bytes, resolvedName)
    }

    AsyncFunction("getImagePixels") Coroutine { uri: String, maxDimension: Int ->
      val context = appContext.reactContext
        ?: throw IllegalStateException("React context is unavailable")
      withContext(Dispatchers.IO) {
        decodePixels(Uri.parse(uri), maxDimension, context)
      }
    }
  }
}
