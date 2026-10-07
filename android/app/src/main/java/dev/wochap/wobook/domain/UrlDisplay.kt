package dev.wochap.wobook.domain

/**
 * Host + path shown in rows: scheme and a leading `www.` dropped, a lone
 * trailing `/` dropped. Mirrors `display_url` in wobook-ffi.
 */
object UrlDisplay {
    fun format(url: String): String {
        var rest = url
        val scheme = rest.indexOf("://")
        if (scheme >= 0) rest = rest.substring(scheme + 3)
        rest = rest.removePrefix("www.")
        if (rest.endsWith('/') && rest.count { it == '/' } == 1) rest = rest.dropLast(1)
        return rest
    }

    /** Host only, for the share sheet's scrim stub and titles. */
    fun host(url: String): String = format(url).substringBefore('/').substringBefore('?')

    private val URL = Regex("""(?i)\b((?:https?://|www\.)[^\s<>"']+)""")

    /** First URL inside shared text, trailing punctuation trimmed. */
    fun extractFirst(text: String): String? =
        URL.find(text)?.value?.trimEnd('.', ',', ';', ':', '!', '?', ')', ']', '}', '"', '\'')

    /** Whether a search query plausibly is a URL (offers "Add a bookmark"). */
    fun looksLikeUrl(query: String): Boolean {
        val q = query.trim()
        if (q.isEmpty() || q.contains(' ')) return false
        if (q.startsWith("http://", true) || q.startsWith("https://", true)) return true
        val host = q.substringBefore('/')
        return host.contains('.') && !host.startsWith('.') && !host.endsWith('.') &&
            host.substringAfterLast('.').let { it.length >= 2 && it.all(Char::isLetter) }
    }
}
