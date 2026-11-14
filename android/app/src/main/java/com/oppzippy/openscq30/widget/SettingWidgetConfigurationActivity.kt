package com.oppzippy.openscq30.widget

import android.appwidget.AppWidgetManager
import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.annotation.DrawableRes
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.datastore.preferences.core.Preferences
import androidx.glance.appwidget.GlanceAppWidgetManager
import androidx.glance.appwidget.state.getAppWidgetState
import androidx.glance.appwidget.state.updateAppWidgetState
import androidx.lifecycle.lifecycleScope
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.features.soundcoredevice.service.ConnectionStatus
import com.oppzippy.openscq30.features.soundcoredevice.service.DeviceService
import com.oppzippy.openscq30.lib.bindings.OpenScq30Device
import com.oppzippy.openscq30.lib.bindings.OpenScq30Session
import com.oppzippy.openscq30.lib.bindings.translateSettingId
import com.oppzippy.openscq30.ui.DeviceServiceConnection
import com.oppzippy.openscq30.ui.sound.DeviceSettingsSnapshot
import com.oppzippy.openscq30.ui.sound.availabilityReasonText
import com.oppzippy.openscq30.ui.sound.title
import com.oppzippy.openscq30.ui.theme.OpenSCQ30Theme
import com.oppzippy.openscq30.ui.utils.LabeledSwitch
import dagger.hilt.android.AndroidEntryPoint
import javax.inject.Inject
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch
import kotlinx.serialization.json.Json

/**
 * Chooses which settings the home screen widget shows, grouped and explained by the shared client rules.
 *
 * The result contract is inherited from the widget provider: [RESULT_OK] with the app widget id when the widget exists,
 * [RESULT_CANCELED] when the user cancels the configuration.
 */
@AndroidEntryPoint
class SettingWidgetConfigurationActivity : ComponentActivity() {
    @Inject
    lateinit var session: OpenScq30Session

    private val deviceServiceConnection = DeviceServiceConnection()
    private val enabledSettingIds: MutableStateFlow<Set<String>?> = MutableStateFlow(null)
    private val isCompactLayout: MutableStateFlow<Boolean?> = MutableStateFlow(null)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        lifecycleScope.launch {
            DeviceService.isRunning.collectLatest { isRunning ->
                if (isRunning) {
                    bindService(
                        Intent(this@SettingWidgetConfigurationActivity, DeviceService::class.java),
                        deviceServiceConnection,
                        0,
                    )
                }
            }
        }

        val appWidgetId =
            intent?.extras?.getInt(AppWidgetManager.EXTRA_APPWIDGET_ID, AppWidgetManager.INVALID_APPWIDGET_ID)
                ?: AppWidgetManager.INVALID_APPWIDGET_ID
        setResult(RESULT_OK, Intent().putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, appWidgetId))

        val manager = GlanceAppWidgetManager(this)
        val glanceId = manager.getGlanceIdBy(appWidgetId)
        val widget = SettingWidget()

        lifecycleScope.launch {
            val preferences = widget.getAppWidgetState<Preferences>(this@SettingWidgetConfigurationActivity, glanceId)
            isCompactLayout.value = preferences[SettingWidget.COMPACT_LAYOUT_KEY]
        }

        lifecycleScope.launch {
            deviceServiceConnection.connectionStatusFlow.collectLatest { connectionStatus ->
                if (connectionStatus is ConnectionStatus.Connected) {
                    val model = connectionStatus.deviceManager.device.model()
                    val preferences =
                        widget.getAppWidgetState<Preferences>(this@SettingWidgetConfigurationActivity, glanceId)
                    enabledSettingIds.value = preferences[SettingWidget.settingIdsKey(model)] ?: emptySet()
                } else {
                    enabledSettingIds.value = null
                }
            }
        }

        lifecycleScope.launch {
            updateAppWidgetState(this@SettingWidgetConfigurationActivity, glanceId) { state ->
                state[SettingWidget.STATE_KEY] =
                    Json.encodeToString<SettingWidgetState>(SettingWidgetState.Disconnected(session.pairedDevices()))
            }
            widget.update(applicationContext, glanceId)

            // we want to exit this activity as quickly as possible, so do this in the background rather than
            // waiting for the service to bind
            sendBroadcast(
                Intent().apply {
                    `package` = packageName
                    action = DeviceService.ACTION_UPDATE_WIDGET
                    putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, appWidgetId)
                },
            )
        }

        enableEdgeToEdge()
        actionBar?.hide()
        setContent {
            Content(
                connectionStatus = deviceServiceConnection.connectionStatusFlow.collectAsState().value,
                enabledSettingIds = enabledSettingIds.collectAsState().value,
                isCompactLayout = isCompactLayout.collectAsState().value ?: false,
                onCancel = { cancel(appWidgetId) },
                onFinish = { finish() },
                onSetSettingIdEnabled = { deviceModel, settingId, isEnabled ->
                    setSettingIdEnabled(
                        appWidgetId,
                        deviceModel,
                        settingId,
                        isEnabled,
                    )
                },
                onCompactLayoutChange = { newValue ->
                    val context = this
                    lifecycleScope.launch {
                        updateAppWidgetState(context, glanceId) { state ->
                            state[SettingWidget.COMPACT_LAYOUT_KEY] = newValue
                        }
                        val preferences = widget.getAppWidgetState<Preferences>(context, glanceId)
                        isCompactLayout.value = preferences[SettingWidget.COMPACT_LAYOUT_KEY]
                        widget.update(context, glanceId)
                    }
                },
            )
        }
    }

    fun cancel(appWidgetId: Int) {
        setResult(RESULT_CANCELED, Intent().putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, appWidgetId))
        finish()
    }

    override fun onDestroy() {
        super.onDestroy()
        try {
            unbindService(deviceServiceConnection)
        } catch (_: IllegalArgumentException) {
            // If the service was never bound, that's fine
        }
    }

    private fun setSettingIdEnabled(appWidgetId: Int, deviceModel: String, settingId: String, isEnabled: Boolean) {
        val context = this
        lifecycleScope.launch {
            val manager = GlanceAppWidgetManager(context)
            val widget = SettingWidget()
            val glanceId = manager.getGlanceIdBy(appWidgetId)
            val settingIdsKey = SettingWidget.settingIdsKey(deviceModel)

            updateAppWidgetState(context, glanceId) { state ->
                state[settingIdsKey] = toggledSettingIds(state[settingIdsKey], settingId, isEnabled)
            }
            val preferences = widget.getAppWidgetState<Preferences>(context, glanceId)
            enabledSettingIds.value = preferences[settingIdsKey] ?: emptySet()

            widget.updateConnectionStatus(
                context,
                session,
                deviceServiceConnection.connectionStatusFlow.value,
                glanceId,
            )
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun Content(
    connectionStatus: ConnectionStatus,
    enabledSettingIds: Set<String>?,
    isCompactLayout: Boolean,
    onCancel: () -> Unit,
    onFinish: () -> Unit,
    onSetSettingIdEnabled: (deviceModel: String, settingId: String, isEnabled: Boolean) -> Unit,
    onCompactLayoutChange: (Boolean) -> Unit,
) {
    val connectedDevice = (connectionStatus as? ConnectionStatus.Connected)?.deviceManager?.device
    OpenSCQ30Theme {
        Scaffold(
            modifier = Modifier.fillMaxSize(),
            topBar = {
                TopAppBar(
                    title = {
                        Text(text = stringResource(R.string.configure_widget))
                    },
                    navigationIcon = {
                        // the client's top bars navigate with the back arrow; here it cancels the configuration
                        IconButton(onClick = onCancel) {
                            Icon(
                                painter = painterResource(R.drawable.arrow_back_24px),
                                contentDescription = stringResource(R.string.cancel),
                            )
                        }
                    },
                    actions = {
                        if (connectedDevice != null) {
                            IconButton(onClick = onFinish) {
                                Icon(
                                    painter = painterResource(R.drawable.check_24px),
                                    contentDescription = stringResource(R.string.confirm),
                                )
                            }
                        }
                    },
                )
            },
        ) { contentPadding ->
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(contentPadding),
            ) {
                when {
                    // With API <28, this will be shown initially when adding a widget (if not connected)
                    // since android:widgetFeatures="configuration_optional" is not available.
                    // Otherwise, the user won't be prompted for configuration until they connect to a device
                    connectedDevice == null -> DisconnectedCard(onCancel = onCancel, onFinish = onFinish)

                    enabledSettingIds == null -> Loading()

                    else -> ConnectedConfiguration(
                        device = connectedDevice,
                        enabledSettingIds = enabledSettingIds,
                        isCompactLayout = isCompactLayout,
                        onSetSettingIdEnabled = onSetSettingIdEnabled,
                        onCompactLayoutChange = onCompactLayoutChange,
                    )
                }
            }
        }
    }
}

@Composable
private fun ConnectedConfiguration(
    device: OpenScq30Device,
    enabledSettingIds: Set<String>,
    isCompactLayout: Boolean,
    onSetSettingIdEnabled: (deviceModel: String, settingId: String, isEnabled: Boolean) -> Unit,
    onCompactLayoutChange: (Boolean) -> Unit,
) {
    val deviceModel = device.model()
    val groups = widgetConfigurationGroups(DeviceSettingsSnapshot.of(device.settingsByCategory()))

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        HelpCard()
        groups.forEach { group ->
            ConfigurationGroupSection(
                group = group,
                enabledSettingIds = enabledSettingIds,
                onToggle = { settingId, isSelected -> onSetSettingIdEnabled(deviceModel, settingId, isSelected) },
            )
        }
        AppearanceSection(
            isCompactLayout = isCompactLayout,
            onCompactLayoutChange = onCompactLayoutChange,
        )
    }
}

@Composable
private fun HelpCard() {
    Surface(
        shape = RoundedCornerShape(24.dp),
        color = MaterialTheme.colorScheme.secondaryContainer,
        contentColor = MaterialTheme.colorScheme.onSecondaryContainer,
        modifier = Modifier.fillMaxWidth(),
    ) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Text(stringResource(R.string.sound_widget), style = MaterialTheme.typography.titleMedium)
            Text(stringResource(R.string.sound_widget_help), style = MaterialTheme.typography.bodySmall)
        }
    }
}

@Composable
private fun ConfigurationGroupSection(
    group: WidgetConfigurationGroup,
    enabledSettingIds: Set<String>,
    onToggle: (settingId: String, isEnabled: Boolean) -> Unit,
) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        GroupHeading(icon = group.group.icon, title = group.group.title())
        Surface(
            shape = RoundedCornerShape(24.dp),
            color = MaterialTheme.colorScheme.surfaceContainerLowest,
            modifier = Modifier.fillMaxWidth(),
        ) {
            Column(
                modifier = Modifier.padding(6.dp),
                verticalArrangement = Arrangement.spacedBy(2.dp),
            ) {
                group.settings.forEach { setting ->
                    SettingRow(
                        label = translateSettingId(setting.settingId),
                        note = setting.note?.let { noteText(it) },
                        isChecked = setting.settingId in enabledSettingIds,
                        isSelectable = setting.canShow,
                        onCheckedChange = { isSelected -> onToggle(setting.settingId, isSelected) },
                    )
                }
            }
        }
    }
}

@Composable
private fun AppearanceSection(isCompactLayout: Boolean, onCompactLayoutChange: (Boolean) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        GroupHeading(icon = R.drawable.view_agenda_24px, title = stringResource(R.string.appearance))
        Surface(
            shape = RoundedCornerShape(24.dp),
            color = MaterialTheme.colorScheme.surfaceContainerLowest,
            modifier = Modifier.fillMaxWidth(),
        ) {
            LabeledSwitch(
                label = stringResource(R.string.compact_layout),
                isChecked = isCompactLayout,
                onCheckedChange = onCompactLayoutChange,
                modifier = Modifier
                    .heightIn(min = 48.dp)
                    .padding(horizontal = 16.dp),
            )
        }
    }
}

@Composable
private fun GroupHeading(@DrawableRes icon: Int, title: String) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(top = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Icon(
            painter = painterResource(icon),
            contentDescription = null,
            modifier = Modifier.size(18.dp),
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(
            text = title,
            style = MaterialTheme.typography.titleMedium,
            fontWeight = FontWeight.SemiBold,
            modifier = Modifier.semantics { heading() },
        )
    }
}

@Composable
private fun SettingRow(
    label: String,
    note: String?,
    isChecked: Boolean,
    isSelectable: Boolean,
    onCheckedChange: (Boolean) -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 48.dp)
            .toggleable(
                value = isChecked,
                enabled = isSelectable,
                role = Role.Switch,
                onValueChange = onCheckedChange,
            )
            .padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Column(
            modifier = Modifier
                .weight(1f)
                .alpha(if (isSelectable) 1f else 0.6f),
            verticalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            Text(text = label, style = MaterialTheme.typography.titleMedium)
            if (note != null) {
                Text(
                    text = note,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        Switch(checked = isChecked, enabled = isSelectable, onCheckedChange = null)
    }
}

@Composable
private fun noteText(note: WidgetConfigurationNote): String = when (note) {
    is WidgetConfigurationNote.Availability -> stringResource(availabilityReasonText(note.reason))
    WidgetConfigurationNote.DependsOnOtherSetting -> stringResource(R.string.widget_config_depends_on_other_setting)
}

@Composable
private fun DisconnectedCard(onCancel: () -> Unit, onFinish: () -> Unit) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Surface(
            shape = RoundedCornerShape(28.dp),
            color = MaterialTheme.colorScheme.surfaceContainerLowest,
            modifier = Modifier.fillMaxWidth(),
        ) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(24.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(20.dp),
            ) {
                Text(
                    text = stringResource(R.string.connect_to_a_device_to_configure_the_widget),
                    style = MaterialTheme.typography.bodyLarge,
                    textAlign = TextAlign.Center,
                )
                FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(12.dp, Alignment.CenterHorizontally),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Button(onClick = onCancel) { Text(stringResource(R.string.cancel)) }
                    Button(onClick = onFinish) { Text(stringResource(R.string.configure_later)) }
                }
            }
        }
    }
}

@Composable
private fun Loading() {
    Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Text(stringResource(R.string.loading))
    }
}
