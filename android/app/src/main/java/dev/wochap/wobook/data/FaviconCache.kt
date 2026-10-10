package dev.wochap.wobook.data

import android.graphics.BitmapFactory
import android.util.LruCache
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import dev.wochap.wobook.domain.Favicons
import dev.wochap.wobook.ffi.FaviconData
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Deferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.Semaphore
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.sync.withPermit
import kotlinx.coroutines.withContext
import java.io.File

/**
 * Per-host icon cache on the device only (android-favicons design D3).
 * Disk: `<key>.img` holds icon bytes, empty `<key>.none` marks "no icon";
 * age is the file's `lastModified`. Memory: LRU of decoded bitmaps.
 * At most 4 fetches run at once and one per host.
 */
class FaviconCache(
    private val dir: File,
    private val fetch: suspend (origin: String) -> FaviconData?,
) {
    private val memory = LruCache<String, ImageBitmap>(200)
    private val lock = Mutex()
    private val inFlight = HashMap<String, Deferred<ImageBitmap?>>()
    private val permits = Semaphore(4)

    /** Decoded bitmap already in memory; never touches disk or network. */
    fun peek(origin: String): ImageBitmap? = memory.get(Favicons.cacheKey(origin))

    /** Memory, then fresh disk entry, then network. */
    suspend fun icon(origin: String): ImageBitmap? {
        val key = Favicons.cacheKey(origin)
        memory.get(key)?.let { return it }
        return load(key, origin, force = false)
    }

    /** Re-fetches ignoring cache ages. Returns whether an icon was found. */
    suspend fun refresh(origin: String): Boolean =
        load(Favicons.cacheKey(origin), origin, force = true) != null

    /** Number of hosts with a cached icon on disk. */
    suspend fun cachedCount(): Int = withContext(Dispatchers.IO) {
        dir.listFiles { f -> f.name.endsWith(".img") }?.size ?: 0
    }

    private suspend fun load(key: String, origin: String, force: Boolean): ImageBitmap? {
        val (deferred, owner) = lock.withLock {
            inFlight[key]?.let { it to false } ?: CompletableDeferred<ImageBitmap?>().also { inFlight[key] = it }.let { it to true }
        }
        if (!owner) return deferred.await()
        val result = runCatching { resolve(key, origin, force) }.getOrNull()
        (deferred as CompletableDeferred).complete(result)
        lock.withLock { inFlight.remove(key) }
        return result
    }

    private suspend fun resolve(key: String, origin: String, force: Boolean): ImageBitmap? {
        val img = File(dir, "$key.img")
        val none = File(dir, "$key.none")
        val now = System.currentTimeMillis()
        if (!force) {
            val cached = withContext(Dispatchers.IO) {
                when {
                    img.exists() && Favicons.isFresh(true, now - img.lastModified()) -> decode(img.readBytes())
                    none.exists() && Favicons.isFresh(false, now - none.lastModified()) -> return@withContext Miss.None
                    else -> return@withContext Miss.Stale
                }
            }
            when (cached) {
                is ImageBitmap -> { memory.put(key, cached); return cached }
                Miss.None -> return null
                else -> Unit // stale or undecodable: fetch again
            }
        }
        val data = permits.withPermit { runCatching { fetch(origin) }.getOrNull() }
        return withContext(Dispatchers.IO) {
            dir.mkdirs()
            val bitmap = data?.bytes?.let(::decode)
            if (bitmap != null && data != null) {
                img.writeBytes(data.bytes)
                none.delete()
                memory.put(key, bitmap)
            } else {
                none.writeBytes(ByteArray(0))
                none.setLastModified(System.currentTimeMillis())
                img.delete()
                memory.remove(key)
            }
            bitmap
        }
    }

    private enum class Miss { None, Stale }

    private fun decode(bytes: ByteArray): ImageBitmap? =
        BitmapFactory.decodeByteArray(bytes, 0, bytes.size)?.asImageBitmap()
}
