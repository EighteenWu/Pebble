package com.qingj01.pebble

import android.app.Activity
import android.app.AlertDialog
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.widget.Toast

object PebbleIntents {
    private const val TAG = "Pebble"

    @JvmStatic
    fun openUrl(context: Context, url: String) {
        val intent = Intent(Intent.ACTION_VIEW, Uri.parse(url))
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        context.applicationContext.startActivity(intent)
    }

    /**
     * Surface a setup() failure on the UI thread. Native panics otherwise
     * force-close the process with no Java dialog.
     */
    @JvmStatic
    fun showStartupError(context: Context, message: String) {
        Log.e(TAG, "startup error: $message")
        val app = context.applicationContext
        val activity = context as? Activity
        Handler(Looper.getMainLooper()).post {
            try {
                Toast.makeText(app, message, Toast.LENGTH_LONG).show()
                if (activity != null && !activity.isFinishing) {
                    AlertDialog.Builder(activity)
                        .setTitle("Pebble failed to start")
                        .setMessage(message)
                        .setPositiveButton(android.R.string.ok, null)
                        .setCancelable(true)
                        .show()
                }
            } catch (error: Exception) {
                Log.e(TAG, "could not show startup error UI", error)
            }
        }
    }
}
