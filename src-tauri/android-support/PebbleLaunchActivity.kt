package com.qingj01.pebble

import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.view.Gravity
import android.webkit.WebView
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import androidx.activity.enableEdgeToEdge
import androidx.appcompat.app.AlertDialog
import androidx.appcompat.app.AppCompatActivity

/**
 * Plain Activity shown before Tauri / WebView / libpebble_lib.so load.
 * If HyperOS kills the Tauri Activity, this screen and pebble-crash.log
 * still prove Java started.
 */
class PebbleLaunchActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        try {
            enableEdgeToEdge()
        } catch (error: Throwable) {
            PebbleCrash.append(this, "PebbleLaunchActivity.enableEdgeToEdge", error)
        }

        PebbleCrash.append(this, "PebbleLaunchActivity.onCreate")

        val status = TextView(this).apply {
            textSize = 18f
            setPadding(48, 48, 48, 24)
            text = "Pebble starting"
        }
        val logView = TextView(this).apply {
            textSize = 12f
            setPadding(48, 0, 48, 48)
            text = previousCrashText()
        }
        val column = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER_HORIZONTAL
            addView(status)
            addView(logView)
        }
        setContentView(ScrollView(this).apply { addView(column) })

        val webViewError = probeWebViewPackage()
        if (webViewError != null) {
            status.text = "Pebble cannot start"
            AlertDialog.Builder(this)
                .setTitle("Pebble failed to start")
                .setMessage(webViewError)
                .setPositiveButton(android.R.string.ok, null)
                .show()
            return
        }

        status.post {
            try {
                PebbleCrash.append(this, "starting MainActivity / Tauri / loadLibrary")
                startActivity(Intent(this, MainActivity::class.java))
            } catch (error: Throwable) {
                PebbleCrash.append(this, "start MainActivity", error)
                status.text = "Pebble failed to start"
                AlertDialog.Builder(this)
                    .setTitle("Pebble failed to start")
                    .setMessage(error.message ?: error.toString())
                    .setPositiveButton(android.R.string.ok, null)
                    .show()
            }
        }
    }

    private fun previousCrashText(): String {
        val previous = PebbleCrash.read(this).trim()
        return if (previous.isEmpty()) {
            "No previous pebble-crash.log"
        } else {
            "Previous pebble-crash.log:\n$previous"
        }
    }

    /**
     * Do not construct a WebView here — that is what Tauri does next.
     * Only report whether a WebView provider exists.
     */
    private fun probeWebViewPackage(): String? {
        return try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                val pkg = WebView.getCurrentWebViewPackage()
                    ?: return "Android System WebView is not available on this device."
                PebbleCrash.append(
                    this,
                    "WebView provider=${pkg.packageName} version=${pkg.versionName}",
                )
            }
            null
        } catch (error: Throwable) {
            PebbleCrash.append(this, "WebView.getCurrentWebViewPackage", error)
            "Android System WebView could not be queried: ${error.message ?: error}"
        }
    }
}
