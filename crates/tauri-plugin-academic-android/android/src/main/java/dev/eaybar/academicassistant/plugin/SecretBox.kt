package dev.eaybar.academicassistant.plugin

import javax.crypto.Cipher
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import kotlin.io.encoding.Base64

/** AES-256-GCM sealing of strings: `base64(12-byte IV + ciphertext + tag)`. */
class SecretBox(private val key: SecretKey) {
    fun seal(plain: String): String {
        val cipher = Cipher.getInstance(TRANSFORMATION)
        // No IV is supplied: the provider generates a fresh random one, which
        // Android Keystore keys require.
        cipher.init(Cipher.ENCRYPT_MODE, key)
        val encrypted = cipher.doFinal(plain.toByteArray(Charsets.UTF_8))
        return Base64.encode(cipher.iv + encrypted)
    }

    /** Returns the plain text, or null on any failure (wrong key, tampered, malformed). */
    fun open(sealed: String): String? =
        try {
            val bytes = Base64.decode(sealed)
            val cipher = Cipher.getInstance(TRANSFORMATION)
            cipher.init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(TAG_BITS, bytes, 0, IV_BYTES))
            String(cipher.doFinal(bytes, IV_BYTES, bytes.size - IV_BYTES), Charsets.UTF_8)
        } catch (_: Exception) {
            null
        }

    private companion object {
        const val TRANSFORMATION = "AES/GCM/NoPadding"
        const val IV_BYTES = 12
        const val TAG_BITS = 128
    }
}
