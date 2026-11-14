package com.oppzippy.openscq30.widget

import android.appwidget.AppWidgetManager
import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.glance.GlanceId
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.GlanceAppWidgetManager
import androidx.glance.appwidget.SizeMode
import androidx.glance.appwidget.provideContent
import androidx.glance.appwidget.state.updateAppWidgetState
import androidx.glance.currentState
import com.oppzippy.openscq30.MainActivity
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.features.soundcoredevice.service.ConnectionStatus
import com.oppzippy.openscq30.features.soundcoredevice.service.DeviceService
import com.oppzippy.openscq30.lib.bindings.ClientBattery
import com.oppzippy.openscq30.lib.bindings.OpenScq30Session
import com.oppzippy.openscq30.lib.bindings.translateDeviceModel
import com.oppzippy.openscq30.lib.bindings.translateSettingId
import com.oppzippy.openscq30.lib.wrapper.Setting
import com.oppzippy.openscq30.lib.wrapper.toValue
import com.oppzippy.openscq30.ui.sound.DeviceSettingsSnapshot
import com.oppzippy.openscq30.ui.theme.isDark
import com.oppzippy.openscq30.ui.theme.systemInDarkTheme
import dagger.hilt.android.EntryPointAccessors
import kotlinx.serialization.json.Json

/**
 * A home screen widget that follows the device the OpenSCL4 client is controlling.
 *
 * Every widget keeps one serialized [DeviceWidgetState] in its own preferences and draws a slice of it; the shared
 * [updateDeviceWidgets] refreshes them all when the connection or the confirmed device snapshot changes.
 */
abstract class DeviceWidget : GlanceAppWidget() {
    final override val sizeMode: SizeMode = SizeMode.Exact

    override suspend fun provideGlance(context: Context, id: GlanceId) {
        // see: [Feature Request] Support for Hilt in GlanceAppWidget https://issuetracker.google.com/issues/218520083
        val hiltEntryPoint = EntryPointAccessors.fromApplication<SettingWidget.SettingWidgetHiltEntryPoint>(
            context.applicationContext,
        )
        val applicationPreferences = hiltEntryPoint.preferences()

        provideContent {
            val widgetPreferences = currentState<Preferences>()
            val state = decodeDeviceWidgetState(widgetPreferences[STATE_KEY])
            val theme by applicationPreferences.themeFlow.collectAsState()
            val palette = widgetPalette(dark = theme.isDark(context.systemInDarkTheme()))
            DeviceContent(context, state, palette)
        }
    }

    @Composable
    protected abstract fun DeviceContent(context: Context, state: DeviceWidgetState?, palette: WidgetPalette)
}

/** The widget state key; each widget instance has its own preferences, so one key is enough. */
internal val STATE_KEY = stringPreferencesKey("widgetState")
internal val LAST_MODEL_KEY = stringPreferencesKey("lastDeviceModel")
internal val PRESET_SLOTS_KEY = stringPreferencesKey("presetSlots")

internal fun decodeDeviceWidgetState(json: String?): DeviceWidgetState? = try {
    json?.let { Json.decodeFromString<DeviceWidgetState>(it) }
} catch (_: Exception) {
    null
}

internal fun decodePresetSlots(json: String?): List<String> = try {
    json?.let { Json.decodeFromString<List<String>>(it) } ?: emptyList()
} catch (_: Exception) {
    emptyList()
}

internal fun encodePresetSlots(slots: List<String>): String = Json.encodeToString(slots)

/** Refreshes every format of the device widgets for [connectionStatus]. */
internal suspend fun updateDeviceWidgets(
    context: Context,
    session: OpenScq30Session,
    connectionStatus: ConnectionStatus,
) {
    for (widget in deviceWidgets()) {
        updateDeviceWidget(context, widget, session, connectionStatus)
    }
}

internal fun deviceWidgets(): List<GlanceAppWidget> = listOf(
    BatteryWidget(),
    ModeToggleWidget(),
    SoundModesWidget(),
    DevicePanelWidget(),
)

private suspend fun updateDeviceWidget(
    context: Context,
    widget: GlanceAppWidget,
    session: OpenScq30Session,
    connectionStatus: ConnectionStatus,
) {
    val manager = GlanceAppWidgetManager(context)
    manager.getGlanceIds(widget.javaClass).forEach { glanceId ->
        var isChanged = false
        updateAppWidgetState(context, glanceId) { preferences ->
            val previous = decodeDeviceWidgetState(preferences[STATE_KEY])
            val state = buildDeviceWidgetState(session, connectionStatus, preferences, previous)
            if (state is DeviceWidgetState.Connected) {
                preferences[LAST_MODEL_KEY] = state.deviceModel
            }
            val json = Json.encodeToString<DeviceWidgetState>(state)
            if (json != preferences[STATE_KEY]) {
                preferences[STATE_KEY] = json
                isChanged = true
            }
        }
        if (isChanged) {
            widget.update(context, glanceId)
        }
    }
}

private suspend fun buildDeviceWidgetState(
    session: OpenScq30Session,
    connectionStatus: ConnectionStatus,
    preferences: Preferences,
    previous: DeviceWidgetState?,
): DeviceWidgetState {
    val lastModel = preferences[LAST_MODEL_KEY] ?: (previous as? DeviceWidgetState.Connected)?.deviceModel.orEmpty()
    val previousName = previous?.deviceName.orEmpty()
    return when (connectionStatus) {
        is ConnectionStatus.Connected -> {
            try {
                val device = connectionStatus.deviceManager.device
                val model = device.model()
                val snapshot = DeviceSettingsSnapshot.of(device.settingsByCategory()).settings
                val quickPresetNames = try {
                    session.quickPresetHandler().use { handler ->
                        handler.quickPresets(device).map { it.name }.toSet()
                    }
                } catch (_: Exception) {
                    null
                }
                val lastAppliedPreset = try {
                    device.lastAppliedPreset()
                } catch (_: IllegalStateException) {
                    null
                }
                connectedWidgetState(
                    deviceName = deviceShortName(model),
                    deviceModel = model,
                    snapshot = snapshot,
                    presetSlots = decodePresetSlots(preferences[PRESET_SLOTS_KEY]),
                    availablePresetNames = quickPresetNames,
                    lastAppliedPreset = lastAppliedPreset,
                )
            } catch (ex: IllegalStateException) {
                Log.w("DeviceWidget", "device was closed, assuming we're disconnected", ex)
                DeviceWidgetState.Disconnected(previousName, lastModel)
            }
        }

        ConnectionStatus.Disconnected, ConnectionStatus.AwaitingConnection ->
            DeviceWidgetState.Disconnected(previousName, lastModel)

        is ConnectionStatus.Connecting -> DeviceWidgetState.Connecting(connectionStatus.macAddress)

        is ConnectionStatus.Recovering -> DeviceWidgetState.Recovering(
            previousName.ifEmpty { connectionStatus.macAddress.orEmpty() },
        )

        is ConnectionStatus.Lost -> DeviceWidgetState.Lost(
            previousName.ifEmpty { connectionStatus.macAddress.orEmpty() },
            connectionStatus.macAddress.orEmpty(),
        )
    }
}

/** Opens the OpenSCL4 client on the device the widget shows. */
internal fun openDeviceAction(context: Context): androidx.glance.action.Action =
    androidx.glance.appwidget.action.actionStartActivity(
        Intent(context, MainActivity::class.java).apply {
            addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP)
        },
    )

/** Retries the lost connection through the device service. */
internal fun retryConnectionAction(context: Context): androidx.glance.action.Action =
    androidx.glance.appwidget.action.actionSendBroadcast(
        Intent().apply {
            `package` = context.packageName
            action = DeviceService.ACTION_RETRY
        },
    )

/** Asks the device service to switch the ambient sound mode. */
internal fun setSoundModeAction(context: Context, modeId: String): androidx.glance.action.Action =
    androidx.glance.appwidget.action.actionSendBroadcast(
        Intent().apply {
            `package` = context.packageName
            action = DeviceService.ACTION_SET_SETTING_VALUE
            putExtra(DeviceService.INTENT_EXTRA_SETTING_ID, "ambientSoundMode")
            putExtra(DeviceService.INTENT_EXTRA_SETTING_VALUE, modeId.toValue())
        },
    )

/** Asks the device service to activate a saved quick preset by name. */
internal fun activatePresetAction(
    context: Context,
    presets: List<WidgetPreset>,
    index: Int,
): androidx.glance.action.Action? {
    val preset = presets.getOrNull(index) ?: return null
    return androidx.glance.appwidget.action.actionSendBroadcast(
        Intent().apply {
            `package` = context.packageName
            action = DeviceService.ACTION_ACTIVATE_PRESET
            putExtra(DeviceService.INTENT_EXTRA_PRESET_NAME, preset.name)
        },
    )
}

/** Opens the instance configuration of the device panel. */
@Composable
internal fun configurePresetsAction(context: Context): androidx.glance.action.Action {
    val glanceId = androidx.glance.LocalGlanceId.current
    val appWidgetId = GlanceAppWidgetManager(context).getAppWidgetId(glanceId)
    return androidx.glance.appwidget.action.actionStartActivity(
        Intent(context, DevicePanelWidgetConfigurationActivity::class.java).apply {
            addFlags(Intent.FLAG_ACTIVITY_NO_HISTORY)
            putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, appWidgetId)
        },
    )
}

/** The label the widget shows for a mode id the device reports. */
internal fun modeLabel(modes: List<WidgetMode>, modeId: String?): String? = modes.firstOrNull { it.id == modeId }?.label
