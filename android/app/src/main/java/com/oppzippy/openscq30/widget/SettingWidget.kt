package com.oppzippy.openscq30.widget

import android.appwidget.AppWidgetManager
import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.core.stringSetPreferencesKey
import androidx.glance.ColorFilter
import androidx.glance.GlanceId
import androidx.glance.GlanceModifier
import androidx.glance.GlanceTheme
import androidx.glance.Image
import androidx.glance.ImageProvider
import androidx.glance.LocalContext
import androidx.glance.LocalGlanceId
import androidx.glance.LocalSize
import androidx.glance.action.Action
import androidx.glance.action.clickable
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.GlanceAppWidgetManager
import androidx.glance.appwidget.RadioButton
import androidx.glance.appwidget.SizeMode
import androidx.glance.appwidget.Switch
import androidx.glance.appwidget.action.actionSendBroadcast
import androidx.glance.appwidget.action.actionStartActivity
import androidx.glance.appwidget.action.actionStartService
import androidx.glance.appwidget.cornerRadius
import androidx.glance.appwidget.lazy.LazyColumn
import androidx.glance.appwidget.lazy.items
import androidx.glance.appwidget.provideContent
import androidx.glance.appwidget.state.updateAppWidgetState
import androidx.glance.background
import androidx.glance.currentState
import androidx.glance.layout.Alignment
import androidx.glance.layout.Box
import androidx.glance.layout.Column
import androidx.glance.layout.ColumnScope
import androidx.glance.layout.Row
import androidx.glance.layout.Spacer
import androidx.glance.layout.fillMaxSize
import androidx.glance.layout.fillMaxWidth
import androidx.glance.layout.height
import androidx.glance.layout.padding
import androidx.glance.layout.size
import androidx.glance.layout.width
import androidx.glance.text.Text
import androidx.glance.text.TextStyle
import androidx.glance.unit.ColorProvider
import com.oppzippy.openscq30.MainActivity
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.features.preferences.EarbudsColor
import com.oppzippy.openscq30.features.soundcoredevice.service.ConnectionStatus
import com.oppzippy.openscq30.features.soundcoredevice.service.DeviceService
import com.oppzippy.openscq30.lib.bindings.OpenScq30Session
import com.oppzippy.openscq30.lib.bindings.translateDeviceModel
import com.oppzippy.openscq30.lib.bindings.translateSettingId
import com.oppzippy.openscq30.lib.bindings.translateValue
import com.oppzippy.openscq30.lib.wrapper.PairedDevice
import com.oppzippy.openscq30.lib.wrapper.Select
import com.oppzippy.openscq30.lib.wrapper.Setting
import com.oppzippy.openscq30.lib.wrapper.Value
import com.oppzippy.openscq30.lib.wrapper.toValue
import com.oppzippy.openscq30.ui.sound.SettingGroup
import com.oppzippy.openscq30.ui.sound.availabilityReasonText
import com.oppzippy.openscq30.ui.sound.deviceArt
import com.oppzippy.openscq30.ui.sound.deviceArtOnCard
import com.oppzippy.openscq30.ui.sound.title
import com.oppzippy.openscq30.ui.theme.OpenSCQ30GlanceTheme
import com.oppzippy.openscq30.ui.theme.prefersDarkTheme
import dagger.hilt.EntryPoint
import dagger.hilt.InstallIn
import dagger.hilt.android.EntryPointAccessors
import dagger.hilt.components.SingletonComponent
import java.math.BigDecimal
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle
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
                isDarkTheme = theme.prefersDarkTheme(),
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
                            val categories = device.settingsByCategory()
                            val snapshot = categories
                                .flatMap { it.settings }
                                .distinctBy { it.settingId }
                                .associate { it.settingId to it.setting }
                            val header = widgetHeader(snapshot)
                            SettingWidgetState.Connected(
                                deviceName = deviceShortName(model),
                                rows = widgetSettings(settingIds, categories, snapshot),
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

@Composable
private fun Content(
    context: Context,
    state: SettingWidgetState?,
    prefersDynamicColor: Boolean,
    earbudsColor: EarbudsColor?,
    isDarkTheme: Boolean,
    isCompactLayout: Boolean,
) {
    OpenSCQ30GlanceTheme(dynamicColor = prefersDynamicColor, darkTheme = isDarkTheme) {
        val isShort = LocalSize.current.height < WidgetDimens.shortHeight
        val compact = isCompactLayout || isShort
        val padding = if (compact) WidgetDimens.compactRootPadding else WidgetDimens.rootPadding

        Column(
            GlanceModifier
                .fillMaxSize()
                .background(GlanceTheme.colors.background)
                .padding(padding),
        ) {
            when (state) {
                null -> DisconnectedScreen(context, emptyList(), compact, isShort, isDarkTheme)

                is SettingWidgetState.Disconnected ->
                    DisconnectedScreen(context, state.pairedDevices, compact, isShort, isDarkTheme)

                is SettingWidgetState.Connecting ->
                    MessageScreen(
                        context = context,
                        title = state.deviceName,
                        message = context.getString(R.string.connecting_to, state.deviceName),
                        compact = compact,
                        isShort = isShort,
                        isDarkTheme = isDarkTheme,
                        showDisconnect = true,
                    )

                is SettingWidgetState.Recovering -> MessageScreen(
                    context = context,
                    title = state.deviceName,
                    message = context.getString(R.string.sound_overview_reconnecting),
                    compact = compact,
                    isShort = isShort,
                    isDarkTheme = isDarkTheme,
                    showDisconnect = true,
                )

                is SettingWidgetState.Lost -> MessageScreen(
                    context = context,
                    title = state.deviceName,
                    message = context.getString(R.string.sound_overview_disconnected),
                    compact = compact,
                    isShort = isShort,
                    isDarkTheme = isDarkTheme,
                    showDisconnect = true,
                ) {
                    WidgetButton(
                        text = context.getString(R.string.sound_overview_retry),
                        onClick = actionSendBroadcast(
                            Intent().apply {
                                `package` = context.packageName
                                action = DeviceService.ACTION_RETRY
                            },
                        ),
                    )
                }

                is SettingWidgetState.ConnectedUnconfigured -> MessageScreen(
                    context = context,
                    title = state.deviceName,
                    message = context.getString(R.string.widget_configure_hint),
                    compact = compact,
                    isShort = isShort,
                    isDarkTheme = isDarkTheme,
                    showDisconnect = true,
                ) {
                    WidgetButton(
                        text = context.getString(R.string.configure_widget),
                        onClick = actionConfigure(context),
                    )
                }

                is SettingWidgetState.Connected -> ConnectedScreen(
                    context = context,
                    state = state,
                    compact = compact,
                    isShort = isShort,
                    earbudsColor = earbudsColor,
                    isDarkTheme = isDarkTheme,
                )
            }
        }
    }
}

/** The device name without the manufacturer, as the client's device card shows it. */
private fun deviceShortName(model: String): String = translateDeviceModel(model).removePrefix("Soundcore ")

@Composable
private fun ColumnScope.DisconnectedScreen(
    context: Context,
    pairedDevices: List<PairedDevice>,
    compact: Boolean,
    isShort: Boolean,
    isDarkTheme: Boolean,
) {
    if (!isShort) {
        WidgetHeader(
            context = context,
            model = null,
            title = context.getString(R.string.disconnected),
            batteryValues = emptyList(),
            oneEarbudOnly = false,
            showDisconnect = false,
            earbudsColor = null,
            isDarkTheme = isDarkTheme,
            compact = compact,
        )
        Spacer(GlanceModifier.height(WidgetDimens.sectionGap))
    }
    if (pairedDevices.isEmpty()) {
        CenteredContent(GlanceModifier.fillMaxWidth().defaultWeight()) {
            Text(
                context.getString(R.string.sound_device_not_connected_hint),
                style = WidgetText.rowLabel.tinted(GlanceTheme.colors.onSurface),
            )
            Spacer(GlanceModifier.height(10.dp))
            WidgetButton(
                text = context.getString(R.string.widget_open_app),
                onClick = actionOpenApp(context),
            )
        }
    } else {
        LazyColumn(GlanceModifier.fillMaxWidth().defaultWeight()) {
            items(pairedDevices) { pairedDevice ->
                PairDeviceRow(context, pairedDevice)
            }
        }
    }
}

@Composable
private fun PairDeviceRow(context: Context, pairedDevice: PairedDevice) {
    Row(
        modifier = GlanceModifier
            .fillMaxWidth()
            .padding(bottom = WidgetDimens.rowGap)
            .clickable(actionConnectToPairedDevice(context, pairedDevice.macAddress))
            .background(GlanceTheme.colors.surface)
            .cornerRadius(WidgetDimens.rowRadius)
            .padding(horizontal = WidgetDimens.rowPaddingHorizontal, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(GlanceModifier.defaultWeight()) {
            Text(
                translateDeviceModel(pairedDevice.model),
                style = WidgetText.rowLabel.tinted(GlanceTheme.colors.onSurface),
                maxLines = 1,
            )
            Text(
                pairedDevice.macAddress,
                style = WidgetText.headerDetail.tinted(GlanceTheme.colors.onSurfaceVariant),
                maxLines = 1,
            )
        }
    }
}

@Composable
private fun ColumnScope.MessageScreen(
    context: Context,
    title: String,
    message: String,
    compact: Boolean,
    isShort: Boolean,
    isDarkTheme: Boolean,
    showDisconnect: Boolean,
    action: (@Composable () -> Unit)? = null,
) {
    if (!isShort) {
        WidgetHeader(
            context = context,
            model = null,
            title = title,
            batteryValues = emptyList(),
            oneEarbudOnly = false,
            showDisconnect = showDisconnect,
            earbudsColor = null,
            isDarkTheme = isDarkTheme,
            compact = compact,
        )
        Spacer(GlanceModifier.height(WidgetDimens.sectionGap))
    }
    CenteredContent(GlanceModifier.fillMaxWidth().defaultWeight()) {
        Text(message, style = WidgetText.rowLabel.tinted(GlanceTheme.colors.onSurface), maxLines = 3)
        if (action != null) {
            Spacer(GlanceModifier.height(10.dp))
            action()
        }
    }
}

@Composable
private fun CenteredContent(modifier: GlanceModifier, content: @Composable () -> Unit) {
    Box(modifier = modifier.padding(horizontal = 12.dp), contentAlignment = Alignment.Center) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            content()
        }
    }
}

@Composable
private fun ColumnScope.ConnectedScreen(
    context: Context,
    state: SettingWidgetState.Connected,
    compact: Boolean,
    isShort: Boolean,
    earbudsColor: EarbudsColor?,
    isDarkTheme: Boolean,
) {
    if (!isShort) {
        WidgetHeader(
            context = context,
            model = state.deviceModel.takeIf { it.isNotEmpty() },
            title = state.deviceName,
            batteryValues = state.batteryValues,
            oneEarbudOnly = state.oneEarbudOnly,
            showDisconnect = true,
            earbudsColor = earbudsColor,
            isDarkTheme = isDarkTheme,
            compact = compact,
        )
        Spacer(GlanceModifier.height(WidgetDimens.sectionGap))
    }
    if (state.rows.isEmpty()) {
        CenteredContent(GlanceModifier.fillMaxWidth().defaultWeight()) {
            Text(
                context.getString(R.string.widget_configure_hint),
                style = WidgetText.rowLabel.tinted(GlanceTheme.colors.onSurface),
            )
            Spacer(GlanceModifier.height(10.dp))
            WidgetButton(
                text = context.getString(R.string.configure_widget),
                onClick = actionConfigure(context),
            )
        }
    } else {
        LazyColumn(GlanceModifier.fillMaxWidth().defaultWeight()) {
            items(state.rows.groups()) { (group, groupRows) ->
                Column {
                    if (!compact) {
                        GroupHeading(context, group)
                    }
                    groupRows.forEach { row ->
                        SettingCard(context, row, compact)
                    }
                }
            }
        }
    }
}

@Composable
private fun GroupHeading(context: Context, group: SettingGroup) {
    Row(
        modifier = GlanceModifier
            .fillMaxWidth()
            .padding(
                start = WidgetDimens.groupHeadingPaddingStart,
                top = WidgetDimens.groupHeadingPaddingTop,
                bottom = WidgetDimens.groupHeadingPaddingBottom,
            ),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Image(
            provider = ImageProvider(group.icon),
            contentDescription = null,
            modifier = GlanceModifier.size(WidgetDimens.groupIconSize),
            colorFilter = ColorFilter.tint(GlanceTheme.colors.onSurfaceVariant),
        )
        Spacer(GlanceModifier.width(4.dp))
        Text(
            group.title(context).uppercase(),
            style = WidgetText.groupHeading.tinted(GlanceTheme.colors.onSurfaceVariant),
            maxLines = 1,
        )
    }
}

@Composable
private fun WidgetHeader(
    context: Context,
    model: String?,
    title: String,
    batteryValues: List<String>,
    oneEarbudOnly: Boolean,
    showDisconnect: Boolean,
    earbudsColor: EarbudsColor?,
    isDarkTheme: Boolean,
    compact: Boolean,
) {
    Row(
        modifier = GlanceModifier
            .fillMaxWidth()
            .background(GlanceTheme.colors.secondaryContainer)
            .cornerRadius(WidgetDimens.headerRadius)
            .padding(
                horizontal = WidgetDimens.headerPaddingHorizontal,
                vertical = WidgetDimens.headerPaddingVertical,
            ),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        val artSize = if (compact) WidgetDimens.compactHeaderArtSize else WidgetDimens.headerArtSize
        val art = model?.let { deviceArt(it) }
        if (art != null) {
            Image(
                provider = ImageProvider(deviceArtOnCard(art, earbudsColor, darkHeader = isDarkTheme)),
                contentDescription = null,
                modifier = GlanceModifier.size(artSize),
            )
        } else {
            Image(
                provider = ImageProvider(R.drawable.openscq30_logo_foreground),
                contentDescription = null,
                modifier = GlanceModifier.size(22.dp),
                colorFilter = ColorFilter.tint(GlanceTheme.colors.onSecondaryContainer),
            )
        }
        Spacer(GlanceModifier.width(6.dp))
        Column(
            modifier = GlanceModifier
                .defaultWeight()
                .clickable(actionOpenApp(context)),
        ) {
            Text(
                title,
                style = WidgetText.headerName.tinted(GlanceTheme.colors.onSecondaryContainer),
                maxLines = 1,
            )
            if (batteryValues.isNotEmpty()) {
                Text(
                    batteryValues.joinToString(" · "),
                    style = WidgetText.headerDetail.tinted(GlanceTheme.colors.onSecondaryContainer),
                    maxLines = 1,
                )
            }
        }
        if (oneEarbudOnly) {
            Spacer(GlanceModifier.width(4.dp))
            Box(
                modifier = GlanceModifier
                    .background(GlanceTheme.colors.tertiaryContainer)
                    .cornerRadius(WidgetDimens.badgeRadius)
                    .padding(horizontal = 4.dp, vertical = 2.dp),
            ) {
                Text(
                    context.getString(R.string.sound_one_earbud_badge),
                    style = WidgetText.badge.tinted(GlanceTheme.colors.onTertiaryContainer),
                    maxLines = 1,
                )
            }
        }
        if (showDisconnect) {
            Spacer(GlanceModifier.width(2.dp))
            Box(
                modifier = GlanceModifier
                    .size(WidgetDimens.disconnectSize)
                    .clickable(actionDisconnect(context)),
                contentAlignment = Alignment.Center,
            ) {
                Image(
                    provider = ImageProvider(R.drawable.baseline_headset_off_24),
                    contentDescription = context.getString(R.string.disconnect),
                    modifier = GlanceModifier.size(WidgetDimens.disconnectIconSize),
                    colorFilter = ColorFilter.tint(GlanceTheme.colors.onSecondaryContainer),
                )
            }
        }
    }
}

@Composable
private fun SettingCard(context: Context, row: WidgetSetting, compact: Boolean) {
    Box(
        modifier = GlanceModifier
            .fillMaxWidth()
            .padding(bottom = WidgetDimens.rowGap)
            .background(GlanceTheme.colors.surface)
            .cornerRadius(WidgetDimens.rowRadius)
            .padding(
                horizontal = WidgetDimens.rowPaddingHorizontal,
                vertical = if (compact) WidgetDimens.compactRowPaddingVertical else WidgetDimens.rowPaddingVertical,
            ),
    ) {
        when (row) {
            is WidgetSetting.Editable -> ShowSetting(context, row.settingId, row.setting, compact)

            is WidgetSetting.ReadOnly -> Column(GlanceModifier.fillMaxWidth()) {
                val value = when (val setting = row.setting) {
                    is Setting.ToggleSetting -> context.getString(
                        if (setting.value) R.string.sound_overview_on else R.string.sound_overview_off,
                    )

                    else -> translateValue(setting, setting.toValue())
                }
                Row(GlanceModifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        modifier = GlanceModifier.defaultWeight(),
                        text = translateSettingId(row.settingId),
                        style = WidgetText.rowLabel.tinted(GlanceTheme.colors.onSurface),
                    )
                    Text(value, style = WidgetText.value.tinted(GlanceTheme.colors.onSurfaceVariant))
                }
                Spacer(GlanceModifier.height(2.dp))
                Text(
                    context.getString(availabilityReasonText(row.reason)),
                    style = WidgetText.reason.tinted(GlanceTheme.colors.onSurfaceVariant),
                )
            }

            is WidgetSetting.Blocked -> Column(GlanceModifier.fillMaxWidth()) {
                Text(
                    translateSettingId(row.settingId),
                    style = WidgetText.rowLabel.tinted(GlanceTheme.colors.onSurface),
                )
                Spacer(GlanceModifier.height(4.dp))
                Box(
                    GlanceModifier
                        .fillMaxWidth()
                        .background(GlanceTheme.colors.tertiaryContainer)
                        .cornerRadius(WidgetDimens.noticeRadius)
                        .padding(horizontal = 6.dp, vertical = 4.dp),
                ) {
                    Text(
                        context.getString(availabilityReasonText(row.reason)),
                        style = WidgetText.reason.tinted(GlanceTheme.colors.onTertiaryContainer),
                    )
                }
            }
        }
    }
}

@Composable
private fun WidgetButton(text: String, onClick: Action) {
    Box(
        modifier = GlanceModifier
            .clickable(onClick)
            .background(GlanceTheme.colors.primary)
            .cornerRadius(WidgetDimens.buttonRadius)
            .padding(
                horizontal = WidgetDimens.buttonPaddingHorizontal,
                vertical = WidgetDimens.buttonPaddingVertical,
            ),
        contentAlignment = Alignment.Center,
    ) {
        Text(text, style = WidgetText.button.tinted(GlanceTheme.colors.onPrimary), maxLines = 1)
    }
}

@Composable
private fun ShowSetting(context: Context, settingId: String, setting: Setting, compact: Boolean) {
    when (setting) {
        is Setting.Action -> WidgetButton(
            text = translateSettingId(settingId),
            onClick = actionSetSettingValue(context, settingId, true.toValue()),
        )

        is Setting.EqualizerSetting -> ReadOnlyValueRow(context, settingId, equalizerSummary(setting))

        is Setting.I32RangeSetting -> ReadOnlyValueRow(context, settingId, setting.value.toString())

        is Setting.ImportStringSetting -> ReadOnlyValueRow(context, settingId, "")

        is Setting.InformationSetting -> ReadOnlyValueRow(context, settingId, setting.value)

        is Setting.SelectSetting -> Select(
            context = context,
            settingId = settingId,
            setting = setting.setting,
            value = setting.value,
            isOptional = false,
            compact = compact,
        )

        is Setting.OptionalSelectSetting -> Select(
            context = context,
            settingId = settingId,
            setting = setting.setting,
            value = setting.value,
            isOptional = true,
            compact = compact,
        )

        is Setting.PresetEqualizerProfileSelect -> Select(
            context = context,
            settingId = settingId,
            setting = setting.select,
            value = setting.value,
            isOptional = true,
            compact = compact,
        )

        is Setting.ModifiableSelectSetting -> Select(
            context = context,
            settingId = settingId,
            setting = setting.setting,
            value = setting.value,
            isOptional = true,
            compact = compact,
        )

        is Setting.MultiSelectSetting -> ReadOnlyValueRow(
            context,
            settingId,
            multiSelectSummary(setting.setting, setting.values),
        )

        is Setting.MultiSelectWithRemoveSetting ->
            ReadOnlyValueRow(context, settingId, multiSelectSummary(setting.setting, setting.values))

        is Setting.ToggleSetting -> Row(
            GlanceModifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                translateSettingId(settingId),
                modifier = GlanceModifier.defaultWeight(),
                style = WidgetText.rowLabel.tinted(GlanceTheme.colors.onSurface),
            )
            Spacer(GlanceModifier.width(8.dp))
            Switch(
                checked = setting.value,
                onCheckedChange = actionSetSettingValue(context, settingId, (!setting.value).toValue()),
            )
        }

        is Setting.HueColorPicker -> Row(
            GlanceModifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                translateSettingId(settingId),
                modifier = GlanceModifier.defaultWeight(),
                style = WidgetText.rowLabel.tinted(GlanceTheme.colors.onSurface),
            )
            Box(GlanceModifier.background(Color.hsv(setting.hue, 1f, 1f)).size(16.dp)) {}
        }

        is Setting.TimeOfDay -> {
            val formatter = DateTimeFormatter.ofLocalizedTime(FormatStyle.SHORT)
            ReadOnlyValueRow(context, settingId, setting.getLocalTime().format(formatter))
        }
    }
}

private fun equalizerSummary(setting: Setting.EqualizerSetting): String {
    val divisor = BigDecimal.ONE.scaleByPowerOfTen(setting.setting.fractionDigits.toInt())
    val scaledValues = setting.value.map { BigDecimal(it.toInt()).divide(divisor) }
    return scaledValues.joinToString(separator = ", ", prefix = "[", postfix = "]")
}

private fun multiSelectSummary(setting: Select, values: List<String>): String {
    val valueSet = values.toHashSet()
    return setting.options.mapIndexedNotNull { index, option ->
        setting.localizedOptions.getOrNull(index).takeIf { valueSet.contains(option) }
    }.joinToString(", ")
}

@Composable
private fun ReadOnlyValueRow(context: Context, settingId: String, value: String) {
    Row(GlanceModifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Text(
            modifier = GlanceModifier.defaultWeight(),
            text = translateSettingId(settingId),
            style = WidgetText.rowLabel.tinted(GlanceTheme.colors.onSurface),
        )
        Spacer(GlanceModifier.width(4.dp))
        Text(value, style = WidgetText.value.tinted(GlanceTheme.colors.onSurfaceVariant))
    }
}

@Composable
private fun Select(
    context: Context,
    settingId: String,
    setting: Select,
    value: String?,
    isOptional: Boolean,
    compact: Boolean,
) {
    val size = LocalSize.current
    val roomForButtons = size.width >= 80.dp * (setting.options.size + if (isOptional) 1 else 0)

    Column(GlanceModifier.fillMaxWidth()) {
        Text(
            translateSettingId(settingId),
            style = WidgetText.selectLabel.tinted(GlanceTheme.colors.onSurfaceVariant),
            maxLines = 1,
        )
        Spacer(GlanceModifier.height(4.dp))
        if (roomForButtons) {
            Row(GlanceModifier.fillMaxWidth()) {
                val buttonModifier = GlanceModifier.defaultWeight()
                if (isOptional) {
                    OptionPill(
                        modifier = buttonModifier.clickable(
                            actionSetSettingValue(context, settingId, Value.OptionalStringValue(null)),
                        ),
                        text = context.getString(R.string.none),
                        isSelected = value == null,
                        compact = compact,
                    )
                }
                var sawFirst = isOptional
                setting.options.zip(setting.localizedOptions).forEach { (option, localizedOption) ->
                    if (sawFirst) {
                        Spacer(GlanceModifier.width(6.dp))
                    } else {
                        sawFirst = true
                    }
                    OptionPill(
                        modifier = buttonModifier.clickable(
                            actionSetSettingValue(context, settingId, option.toValue()),
                        ),
                        text = localizedOption,
                        isSelected = value == option,
                        compact = compact,
                    )
                }
            }
        } else {
            Column {
                if (isOptional) {
                    RadioButton(
                        text = context.getString(R.string.none),
                        style = WidgetText.rowLabel.tinted(GlanceTheme.colors.onSurface),
                        onClick = actionSetSettingValue(context, settingId, Value.OptionalStringValue(null)),
                        checked = value == null,
                    )
                }
                // Columns can have a maximum of 10 children, so chunk into groups of 10 to allow for up to 90 children
                // (not 100 since the preceding None button takes up 1 slot).
                setting.options.zip(setting.localizedOptions).chunked(10).forEach { chunk ->
                    Column {
                        chunk.forEach { (option, localizedOption) ->
                            RadioButton(
                                text = localizedOption,
                                style = WidgetText.rowLabel.tinted(GlanceTheme.colors.onSurface),
                                onClick = actionSetSettingValue(context, settingId, option.toValue()),
                                checked = value == option,
                            )
                        }
                    }
                }
            }
        }
    }
}

/**
 * Use GlanceModifier.clickable to specify the on click action
 */
@Composable
private fun OptionPill(
    modifier: GlanceModifier = GlanceModifier,
    text: String,
    isSelected: Boolean,
    compact: Boolean,
) {
    val backgroundColor = if (isSelected) GlanceTheme.colors.primary else GlanceTheme.colors.surfaceVariant
    val textColor = if (isSelected) GlanceTheme.colors.onPrimary else GlanceTheme.colors.onSurfaceVariant
    Box(
        // Minimal horizontal padding to reduce the likelihood of truncation due to text wrapping
        modifier = modifier
            .padding(horizontal = 2.dp, vertical = if (compact) 2.dp else 4.dp)
            .background(backgroundColor)
            .cornerRadius(WidgetDimens.pillRadius)
            .padding(horizontal = 6.dp, vertical = WidgetDimens.pillPaddingVertical),
        contentAlignment = Alignment.Center,
    ) {
        Text(text, style = WidgetText.pill.copy(color = textColor), maxLines = 1)
    }
}

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

private fun TextStyle.tinted(color: ColorProvider): TextStyle = copy(color = color)

private fun actionOpenApp(context: Context): Action = actionStartActivity(
    Intent(context, MainActivity::class.java).apply {
        addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP)
    },
)

@Composable
private fun actionConfigure(context: Context): Action {
    val glanceId = LocalGlanceId.current
    val appWidgetId = GlanceAppWidgetManager(LocalContext.current).getAppWidgetId(glanceId)
    return actionStartActivity(
        Intent(context, SettingWidgetConfigurationActivity::class.java).apply {
            Intent.FLAG_ACTIVITY_SINGLE_TOP
            setPackage(context.packageName)

            addFlags(Intent.FLAG_ACTIVITY_NO_HISTORY)

            putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, appWidgetId)
        },
    )
}

private fun actionConnectToPairedDevice(context: Context, macAddress: String): Action = actionStartService(
    Intent(context, DeviceService::class.java).apply {
        putExtra(DeviceService.INTENT_EXTRA_MAC_ADDRESS, macAddress)
    },
    isForegroundService = true,
)

private fun actionDisconnect(context: Context): Action = actionSendBroadcast(
    Intent().apply {
        `package` = context.packageName
        action = DeviceService.ACTION_DISCONNECT
    },
)

private fun actionSetSettingValue(context: Context, settingId: String, value: Value): Action = actionSendBroadcast(
    Intent().apply {
        `package` = context.packageName
        action = DeviceService.ACTION_SET_SETTING_VALUE
        putExtra(DeviceService.INTENT_EXTRA_SETTING_ID, settingId)
        putExtra(DeviceService.INTENT_EXTRA_SETTING_VALUE, value)
    },
)
