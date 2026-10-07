package dev.wochap.wobook.domain

/**
 * TagEditor rules (design D5): comma or Enter commits, space is part of the
 * tag, pasted text splits on commas, suggestions match case-insensitively
 * across the whole tag, most used first, at most 8.
 */
object TagRules {
    const val MAX_SUGGESTIONS = 8

    /** Result of typing into the editor's text field. */
    data class Edit(val committed: List<String>, val pending: String)

    /**
     * Splits [input] at commas. Every complete segment is committed; the text
     * after the last comma stays pending. Blank segments are dropped.
     */
    fun onInput(input: String): Edit {
        if (!input.contains(',')) return Edit(emptyList(), input)
        val parts = input.split(',')
        val committed = parts.dropLast(1).map(::clean).filter { it.isNotEmpty() }
        return Edit(committed, parts.last().trimStart())
    }

    /** Enter / IME action: commits the pending text when it isn't blank. */
    fun onEnter(pending: String): Edit {
        val tag = clean(pending)
        return Edit(if (tag.isEmpty()) emptyList() else listOf(tag), "")
    }

    /** Backspace on empty input re-opens the last chip as text. */
    fun onBackspace(chips: List<String>, pending: String): Pair<List<String>, String>? {
        if (pending.isNotEmpty() || chips.isEmpty()) return null
        return chips.dropLast(1) to chips.last()
    }

    /** Lowercase, collapse whitespace, trim. The FFI `parse_tags` is authoritative. */
    fun clean(tag: String): String = tag.trim().lowercase().replace(Regex("\\s+"), " ")

    /** Adds [new] tags not already present, keeping order. */
    fun merge(chips: List<String>, new: List<String>): List<String> =
        (chips + new.filterNot { it in chips }).distinct()

    /**
     * Suggestions for [query] from [known] (tag to count). Empty query lists
     * the most used. Already-chosen tags are excluded.
     */
    fun suggestions(query: String, known: List<Pair<String, Long>>, chosen: List<String>): List<String> {
        val q = clean(query)
        return known
            .filter { (tag, _) -> tag !in chosen && (q.isEmpty() || tag.lowercase().contains(q)) }
            .sortedWith(compareByDescending<Pair<String, Long>> { it.second }.thenBy { it.first })
            .take(MAX_SUGGESTIONS)
            .map { it.first }
    }

    /** Index range of [query] inside [tag] for highlighting, or null. */
    fun matchRange(tag: String, query: String): IntRange? {
        val q = clean(query)
        if (q.isEmpty()) return null
        val start = tag.lowercase().indexOf(q)
        return if (start < 0) null else start until start + q.length
    }
}
