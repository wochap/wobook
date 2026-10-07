package dev.wochap.wobook.domain

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import java.util.Base64

/**
 * Pairing QR payload v1. Mirrors `wobook_sync::pairing::payload` validation so
 * Scan and Paste can reject bad input instantly, before `join_pairing`.
 */
@Serializable
data class QrPayload(
    val v: Int,
    val name: String,
    val id: String,
    val ep: List<String>,
    val s: String,
    val exp: Long,
) {
    /** First endpoint, used for the "Connecting … via" line. */
    val firstEndpoint: String? get() = ep.firstOrNull()

    companion object {
        const val VERSION = 1
        const val TTL_S = 120L
        const val CLOCK_SKEW_S = 30L
        const val MAX_ENDPOINTS = 16

        private val json = Json { ignoreUnknownKeys = true }
        private val HEX_ID = Regex("^[0-9a-f]{64}$")

        /** Parses and validates; the error message is user facing. */
        fun parse(text: String, nowS: Long = System.currentTimeMillis() / 1000): Result<QrPayload> {
            val payload = runCatching { json.decodeFromString(serializer(), text.trim()) }
                .getOrElse { return Result.failure(InvalidPayload("That isn't a wobook pairing code")) }
            return payload.validate(nowS).map { payload }
        }
    }

    fun validate(nowS: Long): Result<Unit> {
        fun fail(message: String) = Result.failure<Unit>(InvalidPayload(message))
        if (v != VERSION) return fail("Unsupported pairing code version $v")
        if (!HEX_ID.matches(id)) return fail("The code's device id is malformed")
        if (ep.isEmpty() || ep.size > MAX_ENDPOINTS) return fail("The code lists no usable address")
        ep.firstOrNull { !isSocketAddress(it) }?.let { return fail("Address $it is not ip:port") }
        val secret = runCatching { Base64.getUrlDecoder().decode(s.trimEnd('=')) }.getOrNull()
        if (secret == null || secret.size != 32) return fail("The code's secret is malformed")
        if (exp + CLOCK_SKEW_S < nowS) return fail("This code has expired")
        if (exp > nowS + TTL_S + CLOCK_SKEW_S) return fail("The code's expiry is too far in the future")
        return Result.success(Unit)
    }
}

class InvalidPayload(message: String) : Exception(message)

/** `a.b.c.d:port` or `[v6]:port`. */
fun isSocketAddress(text: String): Boolean {
    val (host, port) = if (text.startsWith("[")) {
        val close = text.indexOf("]:")
        if (close < 0) return false
        text.substring(1, close) to text.substring(close + 2)
    } else {
        val colon = text.lastIndexOf(':')
        if (colon <= 0) return false
        text.substring(0, colon) to text.substring(colon + 1)
    }
    val p = port.toIntOrNull() ?: return false
    if (p !in 1..65535) return false
    return if (text.startsWith("[")) isIpv6(host) else isIpv4(host)
}

private fun isIpv4(host: String): Boolean {
    val parts = host.split('.')
    return parts.size == 4 && parts.all { part ->
        part.isNotEmpty() && part.length <= 3 && part.all(Char::isDigit) && part.toInt() <= 255 &&
            !(part.length > 1 && part.startsWith('0'))
    }
}

private fun isIpv6(host: String): Boolean {
    if (host.isEmpty() || host.count { it == ':' } < 2) return false
    return host.all { it.isDigit() || it in 'a'..'f' || it in 'A'..'F' || it == ':' || it == '.' }
}

/** 100.64.0.0/10 is the Tailscale CGNAT range. */
fun isTailnetAddress(endpoint: String): Boolean {
    val host = endpoint.substringBeforeLast(':').trim('[', ']')
    val parts = host.split('.').mapNotNull { it.toIntOrNull() }
    if (parts.size == 4) return parts[0] == 100 && parts[1] in 64..127
    return host.lowercase().startsWith("fd7a:115c:a1e0")
}
