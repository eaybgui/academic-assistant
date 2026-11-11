package dev.eaybar.academicassistant.plugin

import android.annotation.SuppressLint
import android.graphics.Color
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.view.View
import android.view.ViewGroup
import android.webkit.CookieManager
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.LinearLayout
import android.widget.TextView
import androidx.activity.ComponentActivity
import androidx.activity.OnBackPressedCallback
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import app.tauri.plugin.JSObject

/**
 * The sign-in screen: a `WebView` on the platform's login page. It reports how it ended
 * through [SignInSession] and shows nothing but the page and a title bar. Nothing here logs
 * or displays a URL, token or cookie.
 */
class SignInActivity : ComponentActivity() {
    private var web: WebView? = null
    private val handler = Handler(Looper.getMainLooper())

    @SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val params = SignInSession.params
        if (params == null) {
            finish()
            return
        }
        SignInSession.activity = this
        val webView = WebView(this)
        web = webView
        setContentView(buildLayout(params, webView))

        webView.settings.javaScriptEnabled = true
        webView.settings.domStorageEnabled = true
        CookieManager.getInstance().setAcceptCookie(true)
        CookieManager.getInstance().setAcceptThirdPartyCookies(webView, true)
        webView.webViewClient = clientFor(params)

        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() = deliver("cancelled")
        })
        handler.postDelayed({ deliver("timeout") }, params.timeoutMs)
        webView.loadUrl(params.url)
    }

    private fun buildLayout(params: SignInParams, webView: WebView): View {
        val title = TextView(this).apply {
            text = params.title
            setBackgroundColor(Color.LTGRAY)
            setTextColor(Color.BLACK)
            setPadding(TITLE_PADDING, TITLE_PADDING, TITLE_PADDING, TITLE_PADDING)
        }
        val root = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        root.addView(title)
        root.addView(webView, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f))
        ViewCompat.setOnApplyWindowInsetsListener(root) { view, insets ->
            val bars = insets.getInsets(
                WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout(),
            )
            view.setPadding(bars.left, bars.top, bars.right, bars.bottom)
            insets
        }
        if (params.silent) {
            root.visibility = View.INVISIBLE
        }
        return root
    }

    private fun clientFor(params: SignInParams) = object : WebViewClient() {
        override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
            val url = request.url.toString()
            val scheme = params.interceptScheme
            if (scheme != null && url.startsWith("$scheme://")) {
                deliver("redirect", url = url)
                return true
            }
            return false
        }

        override fun doUpdateVisitedHistory(view: WebView, url: String, isReload: Boolean) {
            val prefix = params.doneUrlPrefix ?: return
            if (url.startsWith(prefix) && url.length > prefix.length) {
                val cookies = CookieManager.getInstance()
                cookies.flush()
                deliver("signedIn", cookieHeader = cookies.getCookie(params.cookieUrl ?: url) ?: "")
            }
        }
    }

    private fun deliver(status: String, url: String? = null, cookieHeader: String? = null) {
        val reply = JSObject().put("status", status)
        url?.let { reply.put("url", it) }
        cookieHeader?.let { reply.put("cookieHeader", it) }
        SignInSession.finish(reply)
        finish()
    }

    override fun onDestroy() {
        handler.removeCallbacksAndMessages(null)
        web?.destroy()
        web = null
        // Closing the screen any other way (cancelSignIn, the system) counts as cancelled.
        if (SignInSession.activity === this) {
            SignInSession.activity = null
            SignInSession.finish(JSObject().put("status", "cancelled"))
        }
        super.onDestroy()
    }

    private companion object {
        const val TITLE_PADDING = 24
    }
}
