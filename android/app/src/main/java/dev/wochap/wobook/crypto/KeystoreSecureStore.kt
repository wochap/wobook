package dev.wochap.wobook.crypto

import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import dev.wochap.wobook.ffi.SecureKeyStore
import dev.wochap.wobook.ffi.SecureStoreException
import java.io.File
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * `SecureKeyStore` over the Android Keystore (design D9): a non-exportable
 * AES-256-GCM key `wobook-wrap` seals each secret with a random 12-byte IV
 * and AAD `wobook:<kind>:v1`; blobs live at `filesDir/secure/<kind>.bin`.
 */
class KeystoreSecureStore(filesDir: File) : SecureKeyStore {
    private val dir = File(filesDir, "secure").apply { mkdirs() }

    private fun file(kind: String): File {
        require(kind.matches(Regex("[a-z_]+"))) { "bad kind" }
        return File(dir, "$kind.bin")
    }

    override fun load(kind: String): ByteArray? {
        val f = file(kind)
        if (!f.exists()) return null
        val sealed = try {
            f.readBytes()
        } catch (e: Exception) {
            throw SecureStoreException.Failed("read ${f.name}: ${e.message}")
        }
        if (sealed.size <= IV_SIZE) throw SecureStoreException.Corrupt("${f.name} is truncated")
        return try {
            val cipher = Cipher.getInstance(TRANSFORMATION)
            cipher.init(Cipher.DECRYPT_MODE, key(create = false) ?: throw IllegalStateException("wrapping key missing"),
                GCMParameterSpec(TAG_BITS, sealed, 0, IV_SIZE))
            cipher.updateAAD(aad(kind))
            cipher.doFinal(sealed, IV_SIZE, sealed.size - IV_SIZE)
        } catch (e: Exception) {
            throw SecureStoreException.Corrupt("cannot unwrap $kind: ${e.javaClass.simpleName}")
        }
    }

    override fun store(kind: String, bytes: ByteArray) {
        try {
            val cipher = Cipher.getInstance(TRANSFORMATION)
            cipher.init(Cipher.ENCRYPT_MODE, key(create = true))
            cipher.updateAAD(aad(kind))
            val sealed = cipher.iv + cipher.doFinal(bytes)
            val target = file(kind)
            val tmp = File(dir, "${target.name}.tmp")
            tmp.writeBytes(sealed)
            if (!tmp.renameTo(target)) throw IllegalStateException("rename failed")
        } catch (e: SecureStoreException) {
            throw e
        } catch (e: Exception) {
            throw SecureStoreException.Failed("store $kind: ${e.message}")
        }
    }

    override fun remove(kind: String) {
        val f = file(kind)
        if (f.exists() && !f.delete()) throw SecureStoreException.Failed("cannot delete ${f.name}")
    }

    /** Forgets every secret (identity lost → start as a new device). */
    fun clear() {
        dir.listFiles()?.forEach { it.delete() }
    }

    private fun key(create: Boolean): SecretKey? {
        val ks = KeyStore.getInstance(ANDROID_KEY_STORE).apply { load(null) }
        (ks.getKey(ALIAS, null) as? SecretKey)?.let { return it }
        if (!create) return null
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, ANDROID_KEY_STORE)
        generator.init(
            KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setKeySize(256)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setRandomizedEncryptionRequired(true)
                .build(),
        )
        return generator.generateKey()
    }

    private fun aad(kind: String) = "wobook:$kind:v1".encodeToByteArray()

    companion object {
        private const val ANDROID_KEY_STORE = "AndroidKeyStore"
        private const val ALIAS = "wobook-wrap"
        private const val TRANSFORMATION = "AES/GCM/NoPadding"
        private const val IV_SIZE = 12
        private const val TAG_BITS = 128
    }
}
