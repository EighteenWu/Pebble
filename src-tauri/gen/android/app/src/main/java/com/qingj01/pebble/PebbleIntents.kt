package com.qingj01.pebble

import android.content.Context
import android.content.Intent
import android.net.Uri

object PebbleIntents {
    @JvmStatic
    fun openUrl(context: Context, url: String) {
        val intent = Intent(Intent.ACTION_VIEW, Uri.parse(url))
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        context.applicationContext.startActivity(intent)
    }
}
