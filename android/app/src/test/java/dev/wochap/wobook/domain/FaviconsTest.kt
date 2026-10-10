package dev.wochap.wobook.domain

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class FaviconsTest {
    @Test fun letter() {
        assertEquals("E", Favicons.letter("www.example.org"))
        assertEquals("9", Favicons.letter("9gag.com"))
        assertEquals("W", Favicons.letter("wiki.archlinux.org"))
        assertEquals("?", Favicons.letter(""))
    }

    @Test fun stableColour() {
        assertEquals(Favicons.colorIndex("example.org"), Favicons.colorIndex("example.org"))
        assertTrue(Favicons.colorIndex("example.org") in 0..4)
    }

    @Test fun origin() {
        assertEquals("https://www.example.org", Favicons.origin("https://www.example.org/page?q=1"))
        assertEquals("http://127.0.0.1:8080", Favicons.origin("http://127.0.0.1:8080/x"))
        assertNull(Favicons.origin("mailto:a@b.c"))
        assertNull(Favicons.origin("not a url"))
    }

    @Test fun cacheKey() {
        assertEquals("example.org", Favicons.cacheKey("https://example.org"))
        assertEquals("example.org", Favicons.cacheKey("https://example.org:443"))
        assertEquals("127.0.0.1_8080", Favicons.cacheKey("http://127.0.0.1:8080"))
    }

    @Test fun ttl() {
        val day = 24L * 60 * 60 * 1000
        assertTrue(Favicons.isFresh(found = true, ageMs = 3 * day))
        assertFalse(Favicons.isFresh(found = true, ageMs = 31 * day))
        assertTrue(Favicons.isFresh(found = false, ageMs = 6 * day))
        assertFalse(Favicons.isFresh(found = false, ageMs = 8 * day))
    }
}
