package com.oppzippy.openscq30.ui.settings

import android.Manifest
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.util.Log
import android.widget.Toast
import androidx.core.content.ContextCompat
import androidx.core.content.getSystemService
import androidx.lifecycle.ViewModel
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.features.autoconnect.canEnableAutoConnect
import com.oppzippy.openscq30.features.autoconnect.observeCompanionPresenceForAutoconnect
import com.oppzippy.openscq30.features.autoconnect.startAutoConnectService
import com.oppzippy.openscq30.features.autoconnect.stopAutoConnectService
import com.oppzippy.openscq30.features.preferences.EarbudsColor
import com.oppzippy.openscq30.features.preferences.Preferences
import com.oppzippy.openscq30.features.preferences.SoundModesView
import com.oppzippy.openscq30.ui.theme.ThemeType
import com.oppzippy.openscq30.ui.utils.ToastHandler
import dagger.hilt.android.lifecycle.HiltViewModel
import dagger.hilt.android.qualifiers.ApplicationContext
import javax.inject.Inject

@HiltViewModel
class SettingsViewModel @Inject constructor(
    @ApplicationContext private val context: Context,
    private val preferences: Preferences,
    private val toastHandler: ToastHandler,
) : ViewModel() {
    companion object {
        private const val TAG = "SettingsViewModel"
    }

    val autoConnect = preferences.autoConnectFlow
    val theme = preferences.themeFlow
    val dynamicColorEnabled = preferences.dynamicColorFlow
    val earbudsColor = preferences.earbudsColorFlow
    val soundModesView = preferences.soundModesViewFlow
    val showOneEarbudBadge = preferences.oneEarbudBadgeFlow

    fun setAutoConnect(value: Boolean) {
        if (value && !canEnableAutoConnect(Build.VERSION.SDK_INT, hasNotificationPermission())) {
            // The autoconnect service needs a visible notification to run in the background, so promising it
            // without the permission would leave the user with a switch that does nothing after a reboot.
            toastHandler.add(R.string.auto_connect_requires_notifications, Toast.LENGTH_LONG)
            return
        }
        preferences.autoConnect = value
        // The value is already known here, so the helper must not re-read the preference it just wrote.
        if (value) {
            observeCompanionPresenceForAutoconnect(context)
            startAutoConnectService(context)
        } else {
            stopAutoConnectService(context)
        }
    }

    private fun hasNotificationPermission(): Boolean =
        ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) ==
            PackageManager.PERMISSION_GRANTED

    fun setTheme(theme: ThemeType?) {
        preferences.theme = theme
    }

    fun setDynamicColor(enabled: Boolean) {
        preferences.dynamicColor = enabled
    }

    fun setEarbudsColor(color: EarbudsColor?) {
        preferences.earbudsColor = color
    }

    fun setSoundModesView(view: SoundModesView) {
        preferences.soundModesView = view
    }

    fun setShowOneEarbudBadge(show: Boolean) {
        preferences.showOneEarbudBadge = show
    }

    fun copyLogs() {
        Log.i(TAG, "exporting logs")
        val process = try {
            // logs get spammed with View : setRequestedFrameRate 60 times a second, so filter that out
            Runtime.getRuntime().exec(arrayOf("logcat", "-d", "-t", "200", "View:S"))
        } catch (ex: Exception) {
            toastHandler.add("Failed to execute logcat: ${ex.message}", Toast.LENGTH_SHORT)
            Log.e(TAG, "Failed to execute logcat", ex)
            return
        }
        val logs = process.inputStream.bufferedReader().use { reader ->
            reader.useLines { lines -> lines.joinToString("\n") }
        }

        val clipboardManager = context.getSystemService<ClipboardManager>()
        if (clipboardManager != null) {
            clipboardManager.setPrimaryClip(ClipData.newPlainText("OpenSCQ30 logs", logs))
        } else {
            toastHandler.add("Failed to get clipboard manager", Toast.LENGTH_SHORT)
        }
    }

    fun copyLogsUnfiltered() {
        Log.i(TAG, "exporting logs (unfiltered)")
        val process = try {
            // -d: print logs and exit without blocking
            // -t: limit number of lines to print
            Runtime.getRuntime().exec(arrayOf("logcat", "-d", "-t", "4000"))
        } catch (ex: Exception) {
            toastHandler.add("Failed to execute logcat: ${ex.message}", Toast.LENGTH_SHORT)
            Log.e(TAG, "Failed to execute logcat", ex)
            return
        }
        val logs = process.inputStream.bufferedReader().use { reader ->
            reader.useLines { lines -> lines.joinToString("\n") }
        }

        val clipboardManager = context.getSystemService<ClipboardManager>()
        if (clipboardManager != null) {
            clipboardManager.setPrimaryClip(ClipData.newPlainText("OpenSCQ30 logs", logs))
        } else {
            toastHandler.add("Failed to get clipboard manager", Toast.LENGTH_SHORT)
        }
    }
}
