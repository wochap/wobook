package dev.wochap.wobook.domain

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class TagEditorRulesTest {
    @Test fun spaceIsPartOfTag() {
        val e = TagRules.onInput("ui library")
        assertEquals(emptyList<String>(), e.committed)
        assertEquals("ui library", e.pending)
    }

    @Test fun commaCommits() {
        val e = TagRules.onInput("ui library,")
        assertEquals(listOf("ui library"), e.committed)
        assertEquals("", e.pending)
    }

    @Test fun enterCommits() {
        assertEquals(listOf("ai agent"), TagRules.onEnter("  AI   Agent ").committed)
        assertEquals(emptyList<String>(), TagRules.onEnter("   ").committed)
    }

    @Test fun pasteSplitsOnCommas() {
        val e = TagRules.onInput("react, ui library,,css, ty")
        assertEquals(listOf("react", "ui library", "css"), e.committed)
        assertEquals("ty", e.pending)
    }

    @Test fun backspaceReopensLastChip() {
        assertEquals(listOf("a") to "b", TagRules.onBackspace(listOf("a", "b"), ""))
        assertNull(TagRules.onBackspace(listOf("a"), "x"))
        assertNull(TagRules.onBackspace(emptyList(), ""))
    }

    @Test fun suggestionsMatchAcrossSpaces() {
        val known = listOf("ui library" to 3L, "tailwind" to 9L, "ui" to 1L, "docs" to 5L)
        assertEquals(listOf("ui library"), TagRules.suggestions("ui lib", known, emptyList()))
        assertEquals(listOf("tailwind", "docs", "ui library", "ui"), TagRules.suggestions("", known, emptyList()))
        assertEquals(listOf("ui"), TagRules.suggestions("UI", known, listOf("ui library")))
    }

    @Test fun suggestionsCappedAtEight() {
        val known = (1..20).map { "tag$it" to it.toLong() }
        assertEquals(8, TagRules.suggestions("tag", known, emptyList()).size)
    }

    @Test fun matchRange() {
        assertEquals(0..5, TagRules.matchRange("ui library", "ui lib"))
        assertNull(TagRules.matchRange("docs", "x"))
    }
}
