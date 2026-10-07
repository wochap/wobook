package dev.wochap.wobook.domain

import java.text.NumberFormat
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.util.Locale

object Formatting {
    private val day = DateTimeFormatter.ofPattern("d MMM yyyy", Locale.ENGLISH)
    private val dayTime = DateTimeFormatter.ofPattern("d MMM yyyy, HH:mm", Locale.ENGLISH)

    fun date(ms: Long, zone: ZoneId = ZoneId.systemDefault()): String =
        day.format(Instant.ofEpochMilli(ms).atZone(zone))

    fun dateTime(ms: Long, zone: ZoneId = ZoneId.systemDefault()): String =
        dayTime.format(Instant.ofEpochMilli(ms).atZone(zone))

    /** "just now", "2 min ago", "3 h ago", "11 days ago". */
    fun relative(ms: Long?, now: Long = System.currentTimeMillis()): String {
        if (ms == null || ms <= 0) return "never"
        val s = ((now - ms) / 1000).coerceAtLeast(0)
        return when {
            s < 60 -> "just now"
            s < 3600 -> "${s / 60} min ago"
            s < 86_400 -> "${s / 3600} h ago"
            s < 2 * 86_400 -> "yesterday"
            else -> "${s / 86_400} days ago"
        }
    }

    fun count(n: Long): String = NumberFormat.getIntegerInstance(Locale.ENGLISH).format(n)

    fun bytes(n: Long): String = when {
        n < 1024 -> "$n B"
        n < 1024 * 1024 -> String.format(Locale.ENGLISH, "%.1f KB", n / 1024.0)
        else -> String.format(Locale.ENGLISH, "%.1f MB", n / (1024.0 * 1024.0))
    }

    /** "1:28" for a countdown. */
    fun countdown(seconds: Long): String {
        val s = seconds.coerceAtLeast(0)
        return "${s / 60}:${(s % 60).toString().padStart(2, '0')}"
    }

    fun platformLabel(platform: String): String = when (platform.lowercase()) {
        "linux" -> "Linux"
        "android" -> "Android"
        "macos", "darwin" -> "macOS"
        "windows" -> "Windows"
        "" -> "Device"
        else -> platform.replaceFirstChar { it.uppercase() }
    }
}
