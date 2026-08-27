package com.qingj01.pebble

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Wraps the Pebble DEK with an AES/GCM key kept in Android Keystore and
 * persists the ciphertext in private SharedPreferences.
 *
 * Rust loads this class through [Context.getClassLoader] during Tauri
 * setup(). JNI FindClass on a native thread uses the system classloader
 * and cannot see this app package.
 */
object PebbleKeystore {
    private const val PREFS = "pebble_dek_store"
    private const val ANDROID_KEYSTORE = "AndroidKeyStore"
    private const val KEY_ALIAS = "pebble-master-dek-wrap"
    private const val TRANSFORMATION = "AES/GCM/NoPadding"
    private const val GCM_TAG_BITS = 128

    @JvmStatic
    fun getSecret(context: Context, service: String, entry: String): String? {
        val wrapped = prefs(context).getString(prefKey(service, entry), null) ?: return null
        return String(unwrap(decode(wrapped)), Charsets.UTF_8)
    }

    @JvmStatic
    fun setSecret(context: Context, service: String, entry: String, value: String) {
        val wrapped = encode(wrap(value.toByteArray(Charsets.UTF_8)))
        prefs(context).edit().putString(prefKey(service, entry), wrapped).apply()
    }

    @JvmStatic
    fun deleteSecret(context: Context, service: String, entry: String) {
        prefs(context).edit().remove(prefKey(service, entry)).apply()
    }

    private fun prefs(context: Context) =
        context.applicationContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    private fun prefKey(service: String, entry: String) = "$service::$entry"

    private fun wrappingKey(): SecretKey {
        val keyStore = KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }
        (keyStore.getKey(KEY_ALIAS, null) as? SecretKey)?.let { return it }

        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, ANDROID_KEYSTORE)
        generator.init(
            KeyGenParameterSpec.Builder(
                KEY_ALIAS,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
            )
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setRandomizedEncryptionRequired(true)
                .build(),
        )
        return generator.generateKey()
    }

    private fun wrap(plaintext: ByteArray): ByteArray {
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, wrappingKey())
        val iv = cipher.iv
        val ciphertext = cipher.doFinal(plaintext)
        return byteArrayOf(iv.size.toByte()) + iv + ciphertext
    }

    private fun unwrap(packed: ByteArray): ByteArray {
        val ivSize = packed[0].toInt() and 0xff
        val iv = packed.copyOfRange(1, 1 + ivSize)
        val ciphertext = packed.copyOfRange(1 + ivSize, packed.size)
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.DECRYPT_MODE, wrappingKey(), GCMParameterSpec(GCM_TAG_BITS, iv))
        return cipher.doFinal(ciphertext)
    }

    private fun encode(bytes: ByteArray): String = Base64.encodeToString(bytes, Base64.NO_WRAP)

    private fun decode(value: String): ByteArray = Base64.decode(value, Base64.NO_WRAP)
}
