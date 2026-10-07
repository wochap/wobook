package dev.wochap.wobook.domain

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.Base64

class QrPayloadTest {
    private val now = 1_800_000_000L
    private val id = "a".repeat(64)
    private val secret = Base64.getUrlEncoder().withoutPadding().encodeToString(ByteArray(32) { it.toByte() })

    private fun json(
        v: Int = 1,
        id: String = this.id,
        ep: String = "\"192.168.1.40:47390\",\"100.84.12.7:47390\"",
        s: String = secret,
        exp: Long = now + 120,
    ) = """{"v":$v,"name":"gdesktop","id":"$id","ep":[$ep],"s":"$s","exp":$exp}"""

    @Test fun validPayloadParses() {
        val p = QrPayload.parse(json(), now).getOrThrow()
        assertEquals("gdesktop", p.name)
        assertEquals("192.168.1.40:47390", p.firstEndpoint)
    }

    @Test fun notJsonIsRejected() {
        assertTrue(QrPayload.parse("hello", now).isFailure)
    }

    @Test fun wrongVersion() {
        assertTrue(QrPayload.parse(json(v = 2), now).isFailure)
    }

    @Test fun badId() {
        assertTrue(QrPayload.parse(json(id = "A".repeat(64)), now).isFailure)
        assertTrue(QrPayload.parse(json(id = "abc"), now).isFailure)
    }

    @Test fun endpoints() {
        assertTrue(QrPayload.parse(json(ep = ""), now).isFailure)
        assertTrue(QrPayload.parse(json(ep = "\"host:1\""), now).isFailure)
        assertTrue(QrPayload.parse(json(ep = "\"[fe80::1]:47390\""), now).isSuccess)
        assertTrue(QrPayload.parse(json(ep = (1..17).joinToString(",") { "\"10.0.0.$it:1\"" }), now).isFailure)
    }

    @Test fun secretMustBe32Bytes() {
        assertTrue(QrPayload.parse(json(s = "abcd"), now).isFailure)
    }

    @Test fun expiryWindow() {
        assertTrue(QrPayload.parse(json(exp = now - 31), now).isFailure)
        assertTrue(QrPayload.parse(json(exp = now - 29), now).isSuccess)
        assertTrue(QrPayload.parse(json(exp = now + 151), now).isFailure)
    }

    @Test fun tailnetDetection() {
        assertTrue(isTailnetAddress("100.84.12.7:47390"))
        assertFalse(isTailnetAddress("192.168.1.40:47390"))
        assertFalse(isTailnetAddress("100.200.1.1:1"))
    }
}
