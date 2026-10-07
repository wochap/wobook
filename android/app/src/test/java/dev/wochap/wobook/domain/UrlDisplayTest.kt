package dev.wochap.wobook.domain

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class UrlDisplayTest {
    @Test fun hostAndPath() {
        assertEquals("ui.shadcn.com/docs", UrlDisplay.format("https://ui.shadcn.com/docs"))
        assertEquals("example.com", UrlDisplay.format("https://www.example.com/"))
        assertEquals("a.b/c/", UrlDisplay.format("http://a.b/c/"))
        assertEquals("github.com/jarun/buku", UrlDisplay.format("https://github.com/jarun/buku"))
    }

    @Test fun host() {
        assertEquals("wiki.archlinux.org", UrlDisplay.host("https://wiki.archlinux.org/title/Systemd/Timers"))
    }

    @Test fun extractFromSharedText() {
        assertEquals(
            "https://vlcn.io/blog/intro-to-crdts",
            UrlDisplay.extractFirst("A Gentle Introduction to CRDTs https://vlcn.io/blog/intro-to-crdts."),
        )
        assertNull(UrlDisplay.extractFirst("no link here"))
    }

    @Test fun looksLikeUrl() {
        assertTrue(UrlDisplay.looksLikeUrl("example.com/path"))
        assertTrue(UrlDisplay.looksLikeUrl("https://x"))
        assertFalse(UrlDisplay.looksLikeUrl("zig allocators"))
        assertFalse(UrlDisplay.looksLikeUrl("v1.2"))
    }
}
