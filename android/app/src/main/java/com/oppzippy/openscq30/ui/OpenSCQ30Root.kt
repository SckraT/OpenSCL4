@file:OptIn(ExperimentalSharedTransitionApi::class)

package com.oppzippy.openscq30.ui

import androidx.activity.compose.PredictiveBackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.ExperimentalSharedTransitionApi
import androidx.compose.animation.SharedTransitionLayout
import androidx.compose.animation.core.SeekableTransitionState
import androidx.compose.animation.core.rememberTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.listSaver
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.features.preferences.DeviceUiPreferencesViewModel
import com.oppzippy.openscq30.features.preferences.prefersDarkTheme
import com.oppzippy.openscq30.features.preferences.prefersDynamicColor
import com.oppzippy.openscq30.features.soundcoredevice.service.ConnectionStatus
import com.oppzippy.openscq30.ui.deviceselection.ActiveDevice
import com.oppzippy.openscq30.ui.deviceselection.ActiveDeviceState
import com.oppzippy.openscq30.ui.deviceselection.AppControl
import com.oppzippy.openscq30.ui.deviceselection.AppControlSnapshot
import com.oppzippy.openscq30.ui.deviceselection.DeviceNavigationState
import com.oppzippy.openscq30.ui.deviceselection.DeviceSelectionScreen
import com.oppzippy.openscq30.ui.deviceselection.deviceBatteries
import com.oppzippy.openscq30.ui.deviceselection.withMissingEarbud
import com.oppzippy.openscq30.ui.devicesettings.DeviceSettingsScreen
import com.oppzippy.openscq30.ui.sound.SoundConnectionStateScreen
import com.oppzippy.openscq30.ui.sound.isOnlyOneEarbudConnected
import com.oppzippy.openscq30.ui.theme.OpenSCQ30Theme
import com.oppzippy.openscq30.ui.utils.LocalRootAnimatedVisibilityScope
import com.oppzippy.openscq30.ui.utils.LocalSharedTransitionScope
import kotlin.coroutines.cancellation.CancellationException
import kotlinx.coroutines.flow.flowOf

@Composable
fun OpenSCQ30Root(
    viewModel: OpenSCQ30RootViewModel = hiltViewModel(),
    uiPreferences: DeviceUiPreferencesViewModel = hiltViewModel(),
) {
    viewModel.toastHandler.Show()
    OpenSCQ30Theme(darkTheme = prefersDarkTheme(), dynamicColor = prefersDynamicColor()) {
        Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
            val status by viewModel.connectionStatusFlow.collectAsState()
            val deviceSettings by viewModel.deviceSettingsManager.collectAsState()
            val showOneEarbudBadge by uiPreferences.oneEarbudBadgeFlow.collectAsState()
            var navigation by rememberSaveable(stateSaver = deviceNavigationSaver) {
                mutableStateOf(DeviceNavigationState())
            }
            LaunchedEffect(status) {
                navigation = navigation.onAppControl(status.toAppControlSnapshot())
            }
            val showDevice = navigation.showDevice
            val requestedMacAddress = navigation.requestedMacAddress

            // My devices and the device screen: the device card turns into the card on top of the overview and back.
            // A back swipe moves the change along with the finger.
            val screenTransition = remember { SeekableTransitionState(showDevice) }
            LaunchedEffect(showDevice) {
                if (screenTransition.currentState != showDevice || screenTransition.targetState != showDevice) {
                    screenTransition.animateTo(showDevice)
                }
            }
            // Nested NavHosts register their Back handlers after this one and handle their own pages first.
            PredictiveBackHandler(enabled = showDevice) { progress ->
                try {
                    progress.collect { event -> screenTransition.seekTo(event.progress, targetState = false) }
                    navigation = navigation.showList()
                } catch (cancelled: CancellationException) {
                    screenTransition.animateTo(true)
                    throw cancelled
                }
            }
            val transition = rememberTransition(screenTransition, label = "device screen")
            SharedTransitionLayout {
                CompositionLocalProvider(LocalSharedTransitionScope provides this) {
                    transition.AnimatedContent(
                        transitionSpec = { fadeIn(tween(300, delayMillis = 60)) togetherWith fadeOut(tween(220)) },
                    ) { deviceShown ->
                        CompositionLocalProvider(LocalRootAnimatedVisibilityScope provides this) {
                            if (deviceShown) {
                                DeviceScreen(
                                    viewModel = viewModel,
                                    status = status,
                                    requestedMacAddress = requestedMacAddress,
                                    onShowDeviceList = { navigation = navigation.showList() },
                                    onDisconnected = { navigation = navigation.clearDevice() },
                                )
                            } else {
                                val activeSettings by remember(deviceSettings) {
                                    deviceSettings?.allSettingsFlow ?: flowOf(emptyList())
                                }.collectAsState(emptyList())
                                val activeDevice = when (val current = status) {
                                    is ConnectionStatus.Connected -> (
                                        current.macAddress ?: requestedMacAddress
                                        )?.let {
                                        val values = activeSettings.toMap()
                                        val oneEarbudOnly = isOnlyOneEarbudConnected(values)
                                        ActiveDevice(
                                            it,
                                            ActiveDeviceState.Connected,
                                            withMissingEarbud(deviceBatteries(values), oneEarbudOnly),
                                            oneEarbudOnly = oneEarbudOnly && showOneEarbudBadge,
                                        )
                                    }

                                    is ConnectionStatus.Connecting -> ActiveDevice(
                                        current.macAddress,
                                        ActiveDeviceState.Connecting,
                                    )

                                    is ConnectionStatus.Recovering -> ActiveDevice(
                                        current.macAddress,
                                        ActiveDeviceState.Connecting,
                                    )

                                    else -> null
                                }
                                DeviceSelectionScreen(
                                    activeDevice = activeDevice,
                                    onDeviceSelected = {
                                        navigation = navigation.openDevice(it.macAddress)
                                        viewModel.selectDevice(it.macAddress)
                                    },
                                    onDeviceConnectRequested = {
                                        // Keep the list visible until the app actually controls the device.
                                        navigation = navigation.requestControl(it.macAddress)
                                        viewModel.selectDevice(it.macAddress)
                                    },
                                    onDeviceUnpaired = {
                                        viewModel.onDeviceUnpaired(it.macAddress)
                                        navigation = navigation.forgetDevice(it.macAddress)
                                    },
                                )
                            }
                        }
                    }
                }
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

private val deviceNavigationSaver = listSaver<DeviceNavigationState, Any?>(
    save = { listOf(it.showDeviceList, it.requestedMacAddress, it.listConnectMacAddress) },
    restore = { DeviceNavigationState(it[0] as Boolean, it[1] as String?, it[2] as String?) },
)

private fun ConnectionStatus.toAppControlSnapshot(): AppControlSnapshot = when (this) {
    is ConnectionStatus.Connected -> AppControlSnapshot(macAddress, AppControl.Controlling)

    is ConnectionStatus.Connecting -> AppControlSnapshot(macAddress, AppControl.Connecting)

    is ConnectionStatus.Recovering -> AppControlSnapshot(macAddress, AppControl.Connecting)

    // Keep the device screen open so it can show the lost state with Retry.
    is ConnectionStatus.Lost -> AppControlSnapshot(macAddress, AppControl.Controlling)

    ConnectionStatus.AwaitingConnection, ConnectionStatus.Disconnected -> AppControlSnapshot()
}

@Composable
private fun DeviceScreen(
    viewModel: OpenSCQ30RootViewModel,
    status: ConnectionStatus,
    requestedMacAddress: String?,
    onShowDeviceList: () -> Unit,
    onDisconnected: () -> Unit,
) {
    val deviceSettings by viewModel.deviceSettingsManager.collectAsState()
    val deviceManager = status.deviceManager
    val isRequestedDevice = deviceManager != null && status !is ConnectionStatus.Lost && (
        requestedMacAddress == null || status.macAddress == null ||
            requestedMacAddress.equals(status.macAddress, ignoreCase = true)
        )
    val settingsManager = deviceSettings
    if (isRequestedDevice && settingsManager != null) {
        key(deviceManager) {
            DeviceSettingsScreen(
                connectionStatus = status,
                onBack = onShowDeviceList,
                onDisconnect = {
                    viewModel.stopDeviceService()
                    onDisconnected()
                    onShowDeviceList()
                },
                setSettingValues = { settingsManager.setSettingValues(it) },
                setSettingValueForResult = { settingId, value ->
                    settingsManager.setSettingValueForResult(settingId, value)
                },
                pendingWritesFlow = settingsManager.pendingWrites,
                settingWriteErrorsFlow = settingsManager.settingWriteErrors,
                settingsByCategoryFlow = settingsManager.settingsByCategoryFlow,
                quickPresetSlotsFlow = settingsManager.quickPresetSlots,
                onQuickPresetSlotChange = { index, name -> settingsManager.setQuickPresetSlot(index, name) },
                quickPresetsFlow = settingsManager.quickPresetsFlow,
                activateQuickPreset = { settingsManager.activateQuickPreset(it) },
                createQuickPreset = { settingsManager.createQuickPreset(it) },
                deleteQuickPreset = { settingsManager.deleteQuickPreset(it) },
                toggleQuickPresetSetting = { name: String, settingId: String, enabled: Boolean ->
                    settingsManager.toggleQuickPresetSetting(name, settingId, enabled)
                },
                featuredSettingSlotsFlow = settingsManager.featuredSettingSlots,
                onFeaturedSettingSlotChange = { index, name -> settingsManager.setFeaturedSettingSlot(index, name) },
                onQuickPresetLoadCurrentSettings = { settingsManager.createQuickPreset(it) },
                legacyEqualizerProfilesFlow = settingsManager.legacyEqualizerProfilesFlow,
                onMigrateLegacyEqualizerProfiles = { settingsManager.migrateLegacyEqualizerProfiles(it) },
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
            onBack = onShowDeviceList,
            onRetry = { viewModel.retryConnection() },
        )
    }
}
