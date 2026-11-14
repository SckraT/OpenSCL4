package com.oppzippy.openscq30.widget

import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.glance.action.Action
import androidx.glance.appwidget.action.actionSendBroadcast
import com.oppzippy.openscq30.features.soundcoredevice.service.DeviceService
import java.util.UUID

/**
 * Widget and notification commands travel through this exported receiver.
 *
 * A broadcast PendingIntent created by the app and fired by the launcher or the notification shade is treated as if
 * it came from that other process, so the device service's own receiver (registered with RECEIVER_NOT_EXPORTED to
 * keep random apps out) never sees it. This receiver is exported, checks the per-install command token and forwards
 * the command as an in-app broadcast, which the service accepts.
 */
class WidgetCommandReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val action = intent.action
        if (action !in ALLOWED_ACTIONS) {
            Log.w(TAG, "ignoring unknown widget command \$action")
            return
        }
        if (intent.getStringExtra(CommandBroadcasts.EXTRA_TOKEN) != CommandBroadcasts.token(context)) {
            Log.w(TAG, "ignoring a widget command without the app token")
            return
        }
        val forwarded = Intent(action).apply {
            `package` = context.packageName
            intent.extras?.let { putExtras(it) }
        }
        context.sendBroadcast(forwarded)
    }

    private companion object {
        const val TAG = "WidgetCommandReceiver"
    }
}

private val ALLOWED_ACTIONS = setOf(
    DeviceService.ACTION_DISCONNECT,
    DeviceService.ACTION_RETRY,
    DeviceService.ACTION_SEND_NOTIFICATION,
    DeviceService.ACTION_SET_SETTING_VALUE,
    DeviceService.ACTION_QUICK_PRESET,
    DeviceService.ACTION_ACTIVATE_PRESET,
    DeviceService.ACTION_UPDATE_WIDGET,
)

/** Builds the command intents and PendingIntents the widgets and the notification use. */
internal object CommandBroadcasts {
    private const val PREFS_NAME = "widget_command"
    private const val TOKEN_KEY = "token"
    internal const val EXTRA_TOKEN = "com.oppzippy.openscq30.widgetCommandToken"

    /** The per-install token that proves a command came from this app's widget or notification. */
    fun token(context: Context): String {
        val preferences = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
        preferences.getString(TOKEN_KEY, null)?.let { return it }
        val token = UUID.randomUUID().toString()
        preferences.edit().putString(TOKEN_KEY, token).apply()
        return token
    }

    /** The explicit intent to this receiver for [action]. */
    fun intent(context: Context, action: String): Intent = Intent(context, WidgetCommandReceiver::class.java).apply {
        this.action = action
        putExtra(EXTRA_TOKEN, token(context))
    }

    /** A Glance action that sends the command through [WidgetCommandReceiver]. */
    fun glanceAction(context: Context, action: String, configure: (Intent) -> Unit = {}): Action {
        val intent = intent(context, action).also(configure)
        return actionSendBroadcast(intent)
    }

    /** A notification action PendingIntent for [action]. */
    fun pendingIntent(
        context: Context,
        requestCode: Int,
        action: String,
        configure: (Intent) -> Unit = {},
    ): PendingIntent = PendingIntent.getBroadcast(
        context,
        requestCode,
        intent(context, action).also(configure),
        PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
    )
}
