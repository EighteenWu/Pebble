package com.qingj01.pebble

import android.app.Application
import android.os.Handler
import android.os.Looper
import android.widget.Toast

/**
 * Runs before any Activity and before [System.loadLibrary]. A Toast here
 * is the earliest UI we can show on HyperOS if Tauri/WebView dies next.
 */
class PebbleApp : Application() {
    override fun onCreate() {
        super.onCreate()
        installCrashHandler()
        PebbleCrash.append(this, "PebbleApp.onCreate breadcrumb")
        Handler(Looper.getMainLooper()).post {
            try {
                Toast.makeText(applicationContext, "Pebble starting", Toast.LENGTH_LONG).show()
            } catch (error: Exception) {
                PebbleCrash.append(this, "PebbleApp starting Toast", error)
            }
        }
    }

    private fun installCrashHandler() {
        val previous = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { thread, error ->
            try {
                PebbleCrash.append(
                    this,
                    "uncaught on ${thread.name}",
                    error,
                )
            } catch (_: Exception) {
            }
            previous?.uncaughtException(thread, error)
        }
    }
}
