package dev.eaybar.academicassistant.plugin

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.content.SharedPreferences
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.webkit.CookieManager
import android.webkit.WebStorage
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.security.KeyStore
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey

@InvokeArg
class SecretArgs {
    lateinit var key: String
    var value: String? = null
}

@InvokeArg
class SignInArgs {
    lateinit var url: String
    lateinit var title: String
    var interceptScheme: String? = null
    var doneUrlPrefix: String? = null
    var cookieUrl: String? = null
    var silent: Boolean = false
    var timeoutMs: Long = 0
}

/**
 * Android-only helpers. Credentials encrypted under an Android Keystore AES key, stored in app-private
 * preferences. An entry that can no longer be decrypted (key removed, app data
 * restored) reads as missing and is removed.
 */
@TauriPlugin
class AcademicPlugin(private val hostActivity: Activity) : Plugin(hostActivity) {
    private val prefs: SharedPreferences =
        hostActivity.applicationContext.getSharedPreferences(PREFS_FILE, Context.MODE_PRIVATE)
    private val box: SecretBox by lazy { SecretBox(loadOrCreateKey()) }

    @Command
    fun secretGet(invoke: Invoke) {
        val name = invoke.parseArgs(SecretArgs::class.java).key
        val sealed = prefs.getString(name, null)
        val plain = sealed?.let { box.open(it) }
        if (sealed != null && plain == null) {
            prefs.edit().remove(name).commit()
        }
        val result = JSObject()
        plain?.let { result.put("value", it) }
        invoke.resolve(result)
    }

    @Command
    fun secretSet(invoke: Invoke) {
        val args = invoke.parseArgs(SecretArgs::class.java)
        val value = args.value ?: return invoke.reject("Missing value")
        if (prefs.edit().putString(args.key, box.seal(value)).commit()) {
            invoke.resolve()
        } else {
            invoke.reject("Could not save the secret")
        }
    }

    @Command
    fun secretDelete(invoke: Invoke) {
        val name = invoke.parseArgs(SecretArgs::class.java).key
        if (prefs.edit().remove(name).commit()) {
            invoke.resolve()
        } else {
            invoke.reject("Could not delete the secret")
        }
    }

    /** Sends the app to the background, as Back does on a root screen. */
    @Command
    fun moveTaskToBack(invoke: Invoke) {
        hostActivity.runOnUiThread {
            hostActivity.moveTaskToBack(true)
            invoke.resolve()
        }
    }

    /** Opens the sign-in screen; the call is answered when the screen ends. */
    @Command
    fun signIn(invoke: Invoke) {
        val args = invoke.parseArgs(SignInArgs::class.java)
        hostActivity.runOnUiThread {
            if (SignInSession.isOpen) {
                invoke.reject("A sign-in is already open")
                return@runOnUiThread
            }
            SignInSession.begin(
                SignInParams(
                    url = args.url,
                    title = args.title,
                    interceptScheme = args.interceptScheme,
                    doneUrlPrefix = args.doneUrlPrefix,
                    cookieUrl = args.cookieUrl,
                    silent = args.silent,
                    timeoutMs = args.timeoutMs,
                ),
                invoke,
            )
            hostActivity.startActivity(Intent(hostActivity, SignInActivity::class.java))
        }
    }

    /** Finishes an open sign-in screen, which then answers "cancelled". */
    @Command
    fun cancelSignIn(invoke: Invoke) {
        hostActivity.runOnUiThread {
            SignInSession.cancel()
            invoke.resolve()
        }
    }

    /** Removes every cookie and stored page data of the sign-in `WebView`s. */
    @Command
    fun clearSignInData(invoke: Invoke) {
        hostActivity.runOnUiThread {
            WebStorage.getInstance().deleteAllData()
            CookieManager.getInstance().removeAllCookies {
                CookieManager.getInstance().flush()
                invoke.resolve()
            }
        }
    }

    private fun loadOrCreateKey(): SecretKey {
        val keyStore = KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }
        val existing = try {
            keyStore.getKey(KEY_ALIAS, null) as? SecretKey
        } catch (_: Exception) {
            keyStore.deleteEntry(KEY_ALIAS)
            null
        }
        return existing ?: createKey()
    }

    private fun createKey(): SecretKey {
        val spec = KeyGenParameterSpec.Builder(
            KEY_ALIAS,
            KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
        )
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(KEY_BITS)
            .build()
        return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, ANDROID_KEYSTORE)
            .apply { init(spec) }
            .generateKey()
    }

    private companion object {
        const val ANDROID_KEYSTORE = "AndroidKeyStore"
        const val KEY_ALIAS = "academic_assistant_secrets"
        const val KEY_BITS = 256
        const val PREFS_FILE = "academic_secrets"
    }
}
