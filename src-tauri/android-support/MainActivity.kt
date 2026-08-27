package com.qingj01.pebble

import android.os.Bundle
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        // HyperOS / Android 15: enableEdgeToEdge() before super.onCreate can
        // kill the process with no dialog because the Window is not ready.
        try {
            super.onCreate(savedInstanceState)
        } catch (error: Throwable) {
            PebbleCrash.append(this, "MainActivity.super.onCreate", error)
            try {
                PebbleIntents.showStartupError(this, error.message ?: error.toString())
            } catch (_: Throwable) {
            }
            finish()
            return
        }
        try {
            enableEdgeToEdge()
        } catch (error: Throwable) {
            PebbleCrash.append(this, "MainActivity.enableEdgeToEdge skipped", error)
        }
        PebbleCrash.append(this, "MainActivity.onCreate after Tauri super.onCreate")
    }
}
