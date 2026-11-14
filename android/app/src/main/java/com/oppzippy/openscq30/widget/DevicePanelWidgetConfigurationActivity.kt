package com.oppzippy.openscq30.widget

import android.appwidget.AppWidgetManager
import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
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
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import androidx.datastore.preferences.core.Preferences
import androidx.glance.appwidget.GlanceAppWidgetManager
import androidx.glance.appwidget.state.getAppWidgetState
import androidx.glance.appwidget.state.updateAppWidgetState
import androidx.lifecycle.lifecycleScope
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.features.soundcoredevice.service.ConnectionStatus
import com.oppzippy.openscq30.features.soundcoredevice.service.DeviceService
import com.oppzippy.openscq30.lib.bindings.OpenScq30Session
import com.oppzippy.openscq30.ui.DeviceServiceConnection
import com.oppzippy.openscq30.ui.theme.OpenSCQ30Theme
import dagger.hilt.android.AndroidEntryPoint
import javax.inject.Inject
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * Chooses the two quick presets the device panel shows. The slots belong to one widget instance and stay with it when
 * the connection changes; only presets of the current device model are offered.
 */
@AndroidEntryPoint
class DevicePanelWidgetConfigurationActivity : ComponentActivity() {
    @Inject
    lateinit var session: OpenScq30Session

    private val deviceServiceConnection = DeviceServiceConnection()
    private val presetNames = MutableStateFlow<List<String>?>(null)
    private val selectedPresets = MutableStateFlow<List<String>>(emptyList())
    private var glanceId: androidx.glance.GlanceId? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val appWidgetId = intent?.extras
            ?.getInt(AppWidgetManager.EXTRA_APPWIDGET_ID, AppWidgetManager.INVALID_APPWIDGET_ID)
            ?: AppWidgetManager.INVALID_APPWIDGET_ID
        setResult(RESULT_OK, Intent().putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, appWidgetId))

        val manager = GlanceAppWidgetManager(this)
        val id = manager.getGlanceIdBy(appWidgetId)
        glanceId = id
        val widget = DevicePanelWidget()

        lifecycleScope.launch {
            val preferences = widget.getAppWidgetState<Preferences>(this@DevicePanelWidgetConfigurationActivity, id)
            selectedPresets.value = decodePresetSlots(preferences[PRESET_SLOTS_KEY])
        }
        lifecycleScope.launch {
            DeviceService.isRunning.collectLatest { isRunning ->
                if (isRunning) {
                    bindService(
                        Intent(this@DevicePanelWidgetConfigurationActivity, DeviceService::class.java),
                        deviceServiceConnection,
                        0,
                    )
                }
            }
        }
        lifecycleScope.launch {
            deviceServiceConnection.connectionStatusFlow.collectLatest { status ->
                presetNames.value = when (status) {
                    is ConnectionStatus.Connected -> withContext(Dispatchers.Default) {
                        try {
                            val device = status.deviceManager.device
                            session.quickPresetHandler().use { handler ->
                                handler.quickPresets(device).map { it.name }
                            }
                        } catch (_: Exception) {
                            emptyList()
                        }
                    }

                    else -> null
                }
            }
        }

        enableEdgeToEdge()
        actionBar?.hide()
        setContent {
            val status = deviceServiceConnection.connectionStatusFlow.collectAsState().value
            val presets = presetNames.collectAsState().value
            val selected = selectedPresets.collectAsState().value
            Content(
                isConnected = status is ConnectionStatus.Connected,
                presets = presets,
                selected = selected,
                onToggle = { name -> selectedPresets.value = togglePresetSelection(selected, name, MAX_PRESETS) },
                onSave = { save(appWidgetId) },
                onCancel = { cancel(appWidgetId) },
            )
        }
    }

    private fun save(appWidgetId: Int) {
        val id = glanceId ?: return
        lifecycleScope.launch {
            updateAppWidgetState(this@DevicePanelWidgetConfigurationActivity, id) { preferences ->
                preferences[PRESET_SLOTS_KEY] = encodePresetSlots(selectedPresets.value)
            }
            DevicePanelWidget().update(applicationContext, id)
            setResult(RESULT_OK, Intent().putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, appWidgetId))
            finish()
        }
    }

    private fun cancel(appWidgetId: Int) {
        setResult(RESULT_CANCELED, Intent().putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, appWidgetId))
        finish()
    }

    override fun onDestroy() {
        super.onDestroy()
        try {
            unbindService(deviceServiceConnection)
        } catch (_: IllegalArgumentException) {
            // the service was never bound
        }
    }

    private companion object {
        const val MAX_PRESETS = 2
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun Content(
    isConnected: Boolean,
    presets: List<String>?,
    selected: List<String>,
    onToggle: (String) -> Unit,
    onSave: () -> Unit,
    onCancel: () -> Unit,
) {
    OpenSCQ30Theme {
        Scaffold(
            modifier = Modifier.fillMaxSize(),
            topBar = {
                TopAppBar(
                    title = { Text(stringResource(R.string.widget_presets_config_title)) },
                    navigationIcon = {
                        IconButton(onClick = onCancel) {
                            Icon(
                                painter = painterResource(R.drawable.arrow_back_24px),
                                contentDescription = stringResource(R.string.cancel),
                            )
                        }
                    },
                    actions = {
                        if (isConnected) {
                            IconButton(onClick = onSave) {
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
            Box(Modifier.fillMaxSize().padding(contentPadding)) {
                Column(
                    modifier = Modifier
                        .fillMaxSize()
                        .verticalScroll(rememberScrollState())
                        .padding(16.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Surface(
                        shape = RoundedCornerShape(24.dp),
                        color = MaterialTheme.colorScheme.secondaryContainer,
                        contentColor = MaterialTheme.colorScheme.onSecondaryContainer,
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Text(
                            stringResource(R.string.widget_presets_config_help),
                            modifier = Modifier.padding(16.dp),
                            style = MaterialTheme.typography.bodySmall,
                        )
                    }
                    if (!isConnected) {
                        Text(
                            stringResource(R.string.widget_presets_config_connected),
                            style = MaterialTheme.typography.bodyMedium,
                        )
                        Button(onClick = onSave, modifier = Modifier.align(Alignment.Start)) {
                            Text(stringResource(R.string.configure_later))
                        }
                        return@Column
                    }
                    if (presets == null) {
                        Text(stringResource(R.string.loading))
                        return@Column
                    }
                    if (presets.isEmpty()) {
                        Text(stringResource(R.string.widget_presets_config_empty))
                        return@Column
                    }
                    Surface(
                        shape = RoundedCornerShape(24.dp),
                        color = MaterialTheme.colorScheme.surfaceContainerLowest,
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Column(Modifier.padding(6.dp)) {
                            presets.forEach { name ->
                                PresetRow(
                                    name = name,
                                    isSelected = name in selected,
                                    onToggle = { onToggle(name) },
                                )
                            }
                        }
                    }
                    if (selected.size >= 2) {
                        Text(
                            stringResource(R.string.widget_presets_config_max),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun PresetRow(name: String, isSelected: Boolean, onToggle: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .toggleable(value = isSelected, role = Role.Checkbox, onValueChange = { onToggle() })
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Text(name, style = MaterialTheme.typography.bodyLarge)
        Switch(checked = isSelected, onCheckedChange = null)
    }
}
