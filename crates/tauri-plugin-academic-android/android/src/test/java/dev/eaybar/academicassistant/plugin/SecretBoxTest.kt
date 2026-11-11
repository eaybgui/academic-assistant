package dev.eaybar.academicassistant.plugin

import javax.crypto.spec.SecretKeySpec
import kotlin.io.encoding.Base64
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Test

class SecretBoxTest {
    private fun newBox(fill: Byte = 7) = SecretBox(SecretKeySpec(ByteArray(32) { fill }, "AES"))

    @Test
    fun seal_then_open_round_trips() {
        val box = newBox()
        for (plain in listOf("contraseña-ñandú-€", "x".repeat(4096))) {
            assertEquals(plain, box.open(box.seal(plain)))
        }
    }

    @Test
    fun seal_uses_a_fresh_iv() {
        val box = newBox()
        assertNotEquals(box.seal("same text"), box.seal("same text"))
    }

    @Test
    fun open_returns_null_for_a_wrong_key() {
        assertNull(newBox(1).open(newBox(2).seal("secret")))
    }

    @Test
    fun open_returns_null_for_tampered_data() {
        val box = newBox()
        val sealed = box.seal("secret")
        val bytes = Base64.decode(sealed)
        for (index in listOf(0, bytes.size / 2, bytes.size - 1)) {
            val tampered = bytes.copyOf().also { it[index] = (it[index].toInt() xor 1).toByte() }
            assertNull(box.open(Base64.encode(tampered)))
        }
        assertNull(box.open(sealed.take(sealed.length / 2)))
    }

    @Test
    fun open_returns_null_for_malformed_base64() {
        val box = newBox()
        assertNull(box.open("not base64 !!!"))
        assertNull(box.open(""))
    }
}
