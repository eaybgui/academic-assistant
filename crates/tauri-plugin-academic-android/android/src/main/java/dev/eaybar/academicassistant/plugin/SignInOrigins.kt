package dev.eaybar.academicassistant.plugin

import android.content.Context
import android.net.Uri
import android.webkit.WebStorage

/**
 * The web origins (scheme, host and port only) the sign-in screen has loaded, so sign-out can
 * delete the storage those sites left in the shared `WebView` profile. Never records paths,
 * query strings or values, and never records the app's own origin.
 */
object SignInOrigins {
    private const val PREFS_FILE = "academic_sign_in_origins"
    private const val KEY = "origins"
    private val APP_HOSTS = setOf("tauri.localhost", "ipc.localhost")

    /** The origin of an http(s) page that is not the app's own, else null. */
    fun originOf(url: String): String? {
        val uri = Uri.parse(url)
        val scheme = uri.scheme?.lowercase()
        val host = uri.host?.lowercase()
        if ((scheme != "http" && scheme != "https") || host == null || host in APP_HOSTS) return null
        val port = if (uri.port == -1) "" else ":${uri.port}"
        return "$scheme://$host$port"
    }

    fun record(context: Context, url: String) {
        val origin = originOf(url) ?: return
        val prefs = context.getSharedPreferences(PREFS_FILE, Context.MODE_PRIVATE)
        val known = prefs.getStringSet(KEY, emptySet()) ?: emptySet()
        if (origin !in known) {
            prefs.edit().putStringSet(KEY, known + origin).apply()
        }
    }

    /** Deletes the storage of every recorded origin and forgets them. */
    fun clear(context: Context) {
        val prefs = context.getSharedPreferences(PREFS_FILE, Context.MODE_PRIVATE)
        prefs.getStringSet(KEY, emptySet())?.forEach { origin ->
            if (originOf(origin) != null) WebStorage.getInstance().deleteOrigin(origin)
        }
        prefs.edit().remove(KEY).commit()
    }
}
