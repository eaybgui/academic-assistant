package dev.eaybar.academicassistant.plugin

import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject

/** What the sign-in screen should do. */
class SignInParams(
    val url: String,
    val title: String,
    val interceptScheme: String?,
    val doneUrlPrefix: String?,
    val cookieUrl: String?,
    val silent: Boolean,
    val timeoutMs: Long,
)

/**
 * The one open sign-in: its parameters, its screen and the pending `signIn` call.
 * Touched only on the main thread. [finish] answers the call at most once.
 */
object SignInSession {
    var params: SignInParams? = null
        private set
    var activity: SignInActivity? = null
    private var pending: Invoke? = null

    val isOpen: Boolean get() = pending != null

    fun begin(newParams: SignInParams, invoke: Invoke) {
        params = newParams
        pending = invoke
    }

    /** Closes the screen, which reports "cancelled"; answers directly if it never opened. */
    fun cancel() {
        val open = activity
        if (open != null) {
            open.finish()
        } else {
            finish(JSObject().put("status", "cancelled"))
        }
    }

    fun finish(reply: JSObject) {
        pending?.resolve(reply)
        pending = null
        params = null
    }
}
