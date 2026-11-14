package com.oppzippy.openscq30.widget

import android.content.Context
import android.util.Log
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.core.stringSetPreferencesKey
import androidx.glance.GlanceId
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.GlanceAppWidgetManager
import androidx.glance.appwidget.SizeMode
import androidx.glance.appwidget.provideContent
import androidx.glance.appwidget.state.updateAppWidgetState
import androidx.glance.currentState
import com.oppzippy.openscq30.features.soundcoredevice.service.ConnectionStatus
import com.oppzippy.openscq30.lib.bindings.OpenScq30Session
import com.oppzippy.openscq30.lib.bindings.translateDeviceModel
import com.oppzippy.openscq30.lib.wrapper.PairedDevice
import com.oppzippy.openscq30.ui.deviceselection.splitBrand
import com.oppzippy.openscq30.ui.sound.DeviceSettingsSnapshot
import com.oppzippy.openscq30.ui.theme.isDark
import com.oppzippy.openscq30.ui.theme.systemInDarkTheme
import dagger.hilt.EntryPoint
import dagger.hilt.InstallIn
import dagger.hilt.android.EntryPointAccessors
import dagger.hilt.components.SingletonComponent
import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonIgnoreUnknownKeys

@Serializable
internal sealed class SettingWidgetState {
    @Serializable
    data class Disconnected(val pairedDevices: List<PairedDevice>) : SettingWidgetState()

    @Serializable
    data class Connecting(val deviceName: String) : SettingWidgetState()

    @Serializable
    data class Recovering(val deviceName: String) : SettingWidgetState()

    @Serializable
    data class Lost(val deviceName: String, val macAddress: String) : SettingWidgetState()

    @Serializable
    data class ConnectedUnconfigured(val deviceName: String) : SettingWidgetState()

    @Serializable
    @OptIn(ExperimentalSerializationApi::class)
    @JsonIgnoreUnknownKeys
    data class Connected(
        val deviceName: String,
        // Older cached snapshots lack rows. Wait for fresh device data before offering controls.
        val rows: List<WidgetSetting> = emptyList(),
        // Older cached snapshots lack the model, the batteries and the one-earbud flag; a fresh update adds them.
        val deviceModel: String = "",
        val batteryValues: List<String> = emptyList(),
        val oneEarbudOnly: Boolean = false,
    ) : SettingWidgetState()
}

class SettingWidget : GlanceAppWidget() {
    companion object {
        const val TAG = "SettingWidget"
        val STATE_KEY = stringPreferencesKey("setting")
        val COMPACT_LAYOUT_KEY = booleanPreferencesKey("compactLayout")

        fun settingIdsKey(deviceModel: String): Preferences.Key<Set<String>> =
            stringSetPreferencesKey("settingIds-$deviceModel")
    }

    override val sizeMode: SizeMode = SizeMode.Exact

    @EntryPoint
    @InstallIn(SingletonComponent::class)
    interface SettingWidgetHiltEntryPoint {
        fun preferences(): com.oppzippy.openscq30.features.preferences.Preferences
    }

    override suspend fun provideGlance(context: Context, id: GlanceId) {
        // see: [Feature Request] Support for Hilt in GlanceAppWidget https://issuetracker.google.com/issues/218520083
        val hiltEntryPoint =
            EntryPointAccessors.fromApplication<SettingWidgetHiltEntryPoint>(context.applicationContext)
        val applicationPreferences = hiltEntryPoint.preferences()

        provideContent {
            val widgetPreferences = currentState<Preferences>()
            val cachedState = widgetPreferences[STATE_KEY]
            val state: SettingWidgetState? = decodeWidgetState(cachedState)
            if (cachedState != null && state == null) {
                Log.w(TAG, "error decoding widget state")
            }
            val prefersDynamicColor by applicationPreferences.dynamicColorFlow.collectAsState()
            val earbudsColor by applicationPreferences.earbudsColorFlow.collectAsState()
            val theme by applicationPreferences.themeFlow.collectAsState()
            val isCompactLayout = widgetPreferences[COMPACT_LAYOUT_KEY] ?: false

            Content(
                context = context,
                state = state,
                prefersDynamicColor = prefersDynamicColor,
                earbudsColor = earbudsColor,
                isDarkTheme = theme.isDark(systemInDark = context.systemInDarkTheme()),
                isCompactLayout = isCompactLayout,
            )
        }
    }

    suspend fun updatePairedDevices(context: Context, pairedDevices: List<PairedDevice>) {
        val manager = GlanceAppWidgetManager(context)
        manager.getGlanceIds(javaClass).forEach { glanceId ->
            var isChanged = false
            updateAppWidgetState(context, glanceId) { preferences ->
                preferences[STATE_KEY]?.let { stateJson ->
                    isChanged = try {
                        val state = Json.decodeFromString<SettingWidgetState>(stateJson)
                        state is SettingWidgetState.Disconnected
                    } catch (ex: Exception) {
                        Log.w(TAG, "error decoding widget state", ex)
                        true
                    }
                    if (isChanged) {
                        preferences[STATE_KEY] =
                            Json.encodeToString<SettingWidgetState>(
                                SettingWidgetState.Disconnected(pairedDevices = pairedDevices),
                            )
                    }
                }
            }
            if (isChanged) {
                update(context, glanceId)
            }
        }
    }

    suspend fun updateAllConnectionStatus(
        context: Context,
        session: OpenScq30Session,
        connectionStatus: ConnectionStatus,
    ) {
        val manager = GlanceAppWidgetManager(context)

        manager.getGlanceIds(javaClass).forEach { glanceId ->
            updateConnectionStatus(context, session, connectionStatus, glanceId)
        }
    }

    suspend fun updateConnectionStatus(
        context: Context,
        session: OpenScq30Session,
        connectionStatus: ConnectionStatus,
        glanceId: GlanceId,
    ) {
        var isChanged = false
        updateAppWidgetState(context, glanceId) { preferences ->
            val recoveryState = recoveryWidgetState(
                connectionStatus,
                connectionDeviceName(connectionStatus),
            )
            val newState = recoveryState ?: when (connectionStatus) {
                ConnectionStatus.Disconnected,
                ConnectionStatus.AwaitingConnection,
                -> SettingWidgetState.Disconnected(session.pairedDevices())

                is ConnectionStatus.Connecting -> SettingWidgetState.Connecting(connectionStatus.macAddress)

                is ConnectionStatus.Recovering, is ConnectionStatus.Lost ->
                    error("recovery states are handled above")

                is ConnectionStatus.Connected -> {
                    try {
                        val device = connectionStatus.deviceManager.device
                        val model = device.model()
                        val settingIds = preferences[settingIdsKey(model)]

                        if (settingIds.isNullOrEmpty()) {
                            SettingWidgetState.ConnectedUnconfigured(
                                deviceName = translateDeviceModel(model),
                            )
                        } else {
                            val snapshot = DeviceSettingsSnapshot.of(device.settingsByCategory())
                            val header = widgetHeader(snapshot.settings)
                            SettingWidgetState.Connected(
                                deviceName = deviceShortName(model),
                                rows = widgetSettings(settingIds, snapshot),
                                deviceModel = model,
                                batteryValues = header.batteryValues,
                                oneEarbudOnly = header.oneEarbudOnly,
                            )
                        }
                    } catch (ex: IllegalStateException) {
                        Log.w(
                            TAG,
                            "device was closed, assuming we're actually disconnected",
                            ex,
                        )
                        SettingWidgetState.Disconnected(emptyList())
                    }
                }
            }

            val jsonEncodedState = Json.encodeToString(newState)
            if (jsonEncodedState != preferences[STATE_KEY]) {
                preferences[STATE_KEY] = jsonEncodedState
                isChanged = true
            }
        }
        if (isChanged) {
            Log.d(TAG, "glance widget $glanceId changed, updating")
            update(context, glanceId)
        }
    }
}

/** The device name without the manufacturer, as the client's device card shows it. */
private fun deviceShortName(model: String): String = splitBrand(translateDeviceModel(model)).second

/** Reads the state cached in the widget; a snapshot from another version is dropped instead of crashing the widget. */
internal fun decodeWidgetState(json: String?): SettingWidgetState? = try {
    json?.let { Json.decodeFromString<SettingWidgetState>(it) }
} catch (_: Exception) {
    null
}

internal fun recoveryWidgetState(connectionStatus: ConnectionStatus, deviceName: String): SettingWidgetState? =
    when (connectionStatus) {
        is ConnectionStatus.Recovering -> SettingWidgetState.Recovering(deviceName)
        is ConnectionStatus.Lost -> SettingWidgetState.Lost(deviceName, connectionStatus.macAddress)
        else -> null
    }

private fun connectionDeviceName(connectionStatus: ConnectionStatus): String {
    val model = connectionStatus.deviceManager?.let { manager ->
        try {
            deviceShortName(manager.device.model())
        } catch (ex: IllegalStateException) {
            Log.w("SettingWidget", "device was closed, using the address as its name", ex)
            null
        }
    }
    return model ?: connectionStatus.macAddress.orEmpty()
}
