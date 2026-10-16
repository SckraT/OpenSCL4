package com.oppzippy.openscq30.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.features.preferences.prefersDarkTheme
import com.oppzippy.openscq30.features.preferences.prefersDynamicColor
import com.oppzippy.openscq30.features.soundcoredevice.service.ConnectionStatus
import com.oppzippy.openscq30.ui.deviceselection.DeviceSelectionScreen
import com.oppzippy.openscq30.ui.devicesettings.DeviceSettingsScreen
import com.oppzippy.openscq30.ui.sound.SoundConnectionStateScreen
import com.oppzippy.openscq30.ui.theme.OpenSCQ30Theme

@Composable
fun OpenSCQ30Root(viewModel: OpenSCQ30RootViewModel = hiltViewModel()) {
    viewModel.toastHandler.Show()
    OpenSCQ30Theme(darkTheme = prefersDarkTheme(), dynamicColor = prefersDynamicColor()) {
        Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
            val status by viewModel.connectionStatusFlow.collectAsState()
            val deviceSettings by viewModel.deviceSettingsManager.collectAsState()
            var showDeviceList by rememberSaveable { mutableStateOf(false) }
            var requestedMacAddress by rememberSaveable { mutableStateOf<String?>(null) }
            LaunchedEffect(status) {
                when (val current = status) {
                    is ConnectionStatus.Connected -> requestedMacAddress = current.macAddress ?: requestedMacAddress
                    is ConnectionStatus.Connecting -> requestedMacAddress = current.macAddress
                    else -> Unit
                }
            }
            val showDevice = !showDeviceList && (
                requestedMacAddress != null || status is ConnectionStatus.Connected ||
                    status is ConnectionStatus.Connecting
                )
            // Nested NavHosts register their Back handlers after this one and handle their own pages first.
            BackHandler(enabled = showDevice) { showDeviceList = true }
            if (showDevice) {
                val connected = status as? ConnectionStatus.Connected
                val isRequestedDevice = connected != null && (
                    requestedMacAddress == null || connected.macAddress == null ||
                        requestedMacAddress.equals(connected.macAddress, ignoreCase = true)
                    )
                if (isRequestedDevice && deviceSettings != null) {
                    val deviceSettings = requireNotNull(deviceSettings)
                    key(connected.deviceManager) {
                        DeviceSettingsScreen(
                            connectionStatus = connected,
                            onBack = { showDeviceList = true },
                            onDisconnect = {
                                viewModel.stopDeviceService()
                                requestedMacAddress = null
                                showDeviceList = true
                            },
                            setSettingValues = { deviceSettings.setSettingValues(it) },
                            setSettingValueForResult = { settingId, value ->
                                deviceSettings.setSettingValueForResult(settingId, value)
                            },
                            allSettingsFlow = deviceSettings.allSettingsFlow,
                            categoryIdsFlow = deviceSettings.categoryIdsFlow,
                            getSettingsInCategoryFlow = { deviceSettings.getSettingsInCategoryFlow(it) },
                            quickPresetSlotsFlow = deviceSettings.quickPresetSlots,
                            onQuickPresetSlotChange = { index, name ->
                                deviceSettings.setQuickPresetSlot(index, name)
                            },
                            quickPresetsFlow = deviceSettings.quickPresetsFlow,
                            activateQuickPreset = { deviceSettings.activateQuickPreset(it) },
                            createQuickPreset = { deviceSettings.createQuickPreset(it) },
                            deleteQuickPreset = { deviceSettings.deleteQuickPreset(it) },
                            toggleQuickPresetSetting = { name: String, settingId: String, enabled: Boolean ->
                                deviceSettings.toggleQuickPresetSetting(name, settingId, enabled)
                            },
                            featuredSettingSlotsFlow = deviceSettings.featuredSettingSlots,
                            onFeaturedSettingSlotChange = { index, name ->
                                deviceSettings.setFeaturedSettingSlot(index, name)
                            },
                            onQuickPresetLoadCurrentSettings = { deviceSettings.createQuickPreset(it) },
                            legacyEqualizerProfilesFlow = deviceSettings.legacyEqualizerProfilesFlow,
                            onMigrateLegacyEqualizerProfile = { deviceSettings.migrateLegacyEqualizerProfile(it) },
                        )
                    }
                } else {
                    val visibleStatus = if (status is ConnectionStatus.Connected) {
                        ConnectionStatus.AwaitingConnection
                    } else {
                        status
                    }
                    SoundConnectionStateScreen(
                        status = visibleStatus,
                        onBack = { showDeviceList = true },
                        onRetry = { viewModel.retryConnection() },
                    )
                }
            } else {
                DeviceSelectionScreen(onDeviceSelected = {
                    requestedMacAddress = it.macAddress
                    showDeviceList = false
                    viewModel.selectDevice(it.macAddress)
                }, onDeviceUnpaired = {
                    viewModel.onDeviceUnpaired(it.macAddress)
                    if (requestedMacAddress.equals(it.macAddress, ignoreCase = true)) requestedMacAddress = null
                })
            }
            if (viewModel.version2BreakingChangesMessage.shouldShow.collectAsState(false).value) {
                WhatsNew(
                    messageHtml = stringResource(R.string.version_2_breaking_changes),
                    onClose = { viewModel.version2BreakingChangesMessage.setShown() },
                )
            }
        }
    }
}
