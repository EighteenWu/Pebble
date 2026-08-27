package com.qingj01.pebble

import android.content.Context
import android.util.Log
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * Process-local crash / breadcrumb log. Written before native load so a
 * HyperOS silent kill still leaves a file the next launch can show.
 */
object PebbleCrash {
    const val FILE_NAME = "pebble-crash.log"
    private const val TAG = "Pebble"

    @JvmStatic
    fun append(context: Context, message: String) {
        val line = "${timestamp()} $message\n"
        Log.e(TAG, message)
        val app = context.applicationContext
        writeTo(File(app.filesDir, FILE_NAME), line)
        app.getExternalFilesDir(null)?.let { writeTo(File(it, FILE_NAME), line) }
    }

    @JvmStatic
    fun append(context: Context, where: String, error: Throwable) {
        append(context, "$where: ${error.javaClass.name}: ${error.message}\n${error.stackTraceToString()}")
    }

    @JvmStatic
    fun read(context: Context): String {
        val internal = File(context.applicationContext.filesDir, FILE_NAME)
        return if (internal.exists()) internal.readText() else ""
    }

    private fun writeTo(file: File, line: String) {
        try {
            file.appendText(line)
        } catch (ignored: Exception) {
            Log.e(TAG, "could not write ${file.absolutePath}", ignored)
        }
    }

    private fun timestamp(): String {
        return SimpleDateFormat("yyyy-MM-dd HH:mm:ss.SSS", Locale.US).format(Date())
    }
}
