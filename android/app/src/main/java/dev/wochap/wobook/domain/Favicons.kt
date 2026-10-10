package dev.wochap.wobook.domain

import java.net.URI

/** Pure favicon rules: origin, cache key, TTL, letter tile (android-favicons design D3, D4). */
object Favicons {
    const val FOUND_TTL_MS = 30L * 24 * 60 * 60 * 1000
    const val NONE_TTL_MS = 7L * 24 * 60 * 60 * 1000

    /** `scheme://host[:port]` of a bookmark URL, or null when it has no host. */
    fun origin(url: String): String? {
        val uri = runCatching { URI(url.trim()) }.getOrNull() ?: return null
        val scheme = uri.scheme?.lowercase() ?: return null
        if (scheme != "http" && scheme != "https") return null
        val host = uri.host?.lowercase()?.takeIf { it.isNotEmpty() } ?: return null
        return if (uri.port == -1) "$scheme://$host" else "$scheme://$host:${uri.port}"
    }

    /** Host of an origin, lowercased. */
    fun host(origin: String): String =
        origin.substringAfter("://").substringBefore('/').let { hostPort ->
            if (hostPort.startsWith('[')) hostPort.substringBefore(']') + "]" else hostPort.substringBefore(':')
        }.lowercase()

    /** File name stem: host, plus `_<port>` for a non-default port, limited to `[a-z0-9._-]`. */
    fun cacheKey(origin: String): String {
        val scheme = origin.substringBefore("://").lowercase()
        val hostPort = origin.substringAfter("://").substringBefore('/').lowercase()
        val host = host(origin)
        val port = hostPort.removePrefix(host).removePrefix(":").toIntOrNull()
        val default = if (scheme == "https") 443 else 80
        val raw = if (port == null || port == default) host else "${host}_$port"
        return raw.map { if (it in 'a'..'z' || it in '0'..'9' || it == '.' || it == '_' || it == '-') it else '_' }
            .joinToString("")
    }

    /** Whether a cache entry written `ageMs` ago is still usable. */
    fun isFresh(found: Boolean, ageMs: Long): Boolean =
        ageMs in 0 until if (found) FOUND_TTL_MS else NONE_TTL_MS

    /** First letter or digit of the host without a leading `www.`, uppercased. */
    fun letter(host: String): String {
        val h = host.lowercase().removePrefix("www.")
        return h.firstOrNull { it in 'a'..'z' || it in '0'..'9' }?.uppercase() ?: "?"
    }

    /** Index into the 5-colour tile palette; stable for a host. */
    fun colorIndex(host: String): Int = host.lowercase().hashCode().mod(5)
}
