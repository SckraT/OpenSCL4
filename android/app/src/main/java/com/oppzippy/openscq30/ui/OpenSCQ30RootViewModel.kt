package com.oppzippy.openscq30.ui

import android.app.Application
import android.util.Log
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.oppzippy.openscq30.android.IntentFactory
import com.oppzippy.openscq30.features.equalizer.storage.LegacyEqualizerProfileDao
import com.oppzippy.openscq30.features.soundcoredevice.service.ConnectionStatus
import com.oppzippy.openscq30.features.soundcoredevice.service.DeviceService
import com.oppzippy.openscq30.features.soundcoredevice.service.isSameSession
import com.oppzippy.openscq30.features.statusnotification.storage.FeaturedSettingSlotDao
import com.oppzippy.openscq30.features.statusnotification.storage.QuickPresetSlotDao
import com.oppzippy.openscq30.features.whatsnew.Version2BreakingChangesMessage
import com.oppzippy.openscq30.lib.bindings.OpenScq30Session
import com.oppzippy.openscq30.ui.utils.ToastHandler
import dagger.hilt.android.lifecycle.HiltViewModel
import javax.inject.Inject
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.job
import kotlinx.coroutines.launch

@HiltViewModel
class OpenSCQ30RootViewModel @Inject constructor(
    private val session: OpenScq30Session,
    private val application: Application,
    private val intentFactory: IntentFactory,
    private val quickPresetSlotDao: QuickPresetSlotDao,
    private val featuredSettingSlotDao: FeaturedSettingSlotDao,
    private val legacyEqualizerProfileDao: LegacyEqualizerProfileDao,
    val version2BreakingChangesMessage: Version2BreakingChangesMessage,
    val toastHandler: ToastHandler = ToastHandler(),
) : AndroidViewModel(application) {
    private var isDeviceServiceBound = false
    private var lastSelectedMacAddress: String? = null
    private var pendingMacAddress: String? = null
    private val deviceServiceConnection = DeviceServiceConnection { unbindDeviceService() }
    private val visibleConnectionStatusFlow = MutableStateFlow<ConnectionStatus>(ConnectionStatus.Disconnected)
    val connectionStatusFlow = visibleConnectionStatusFlow.asStateFlow()

    var deviceSettingsManager = MutableStateFlow<DeviceSettingsManager?>(null)
        private set

    companion object {
        const val TAG = "OpenSCQ30RootViewModel"
    }

    init {
        viewModelScope.launch {
            DeviceService.isRunning.collectLatest { isRunning ->
                if (isRunning) {
                    bindDeviceService()
                } else {
                    unbindDeviceService()
                }
            }
        }

        viewModelScope.launch {
            deviceServiceConnection.connectionStatusFlow.collect {
                when (it) {
                    is ConnectionStatus.Connecting -> {
                        rememberConnectionAddress(it.macAddress)
                    }

                    is ConnectionStatus.Connected -> {
                        rememberConnectionAddress(it.macAddress)
                    }

                    is ConnectionStatus.Recovering -> {
                        rememberConnectionAddress(it.macAddress)
                    }

                    is ConnectionStatus.Lost -> {
                        rememberConnectionAddress(it.macAddress)
                    }

                    ConnectionStatus.Disconnected -> {
                        pendingMacAddress = null
                        unbindDeviceService()
                    }

                    ConnectionStatus.AwaitingConnection -> Unit
                }
                updateVisibleConnectionStatus()
                keepOrReplaceSettingsManager(it)
            }
        }
    }

    /**
     * Recovering and lost keep the device object, so the settings and the current screen must survive. Only
     * an explicit close or a different device replaces or clears the settings manager.
     */
    private fun keepOrReplaceSettingsManager(status: ConnectionStatus) {
        val manager = status.deviceManager
        if (manager == null) {
            deviceSettingsManager.value?.close()
            deviceSettingsManager.value = null
        } else if (deviceSettingsManager.value?.deviceManager !== manager) {
            deviceSettingsManager.value?.close()
            deviceSettingsManager.value = DeviceSettingsManager(
                session = session,
                deviceManager = manager,
                parent = viewModelScope.coroutineContext.job,
                quickPresetSlotDao = quickPresetSlotDao,
                featuredSettingSlotDao = featuredSettingSlotDao,
                legacyEqualizerProfileDao = legacyEqualizerProfileDao,
                toastHandler = toastHandler,
            )
        }
    }

    private fun rememberConnectionAddress(macAddress: String?) {
        // An existing binding can report its old connection before the next start intent is handled.
        if (pendingMacAddress == null || macAddress == null ||
            pendingMacAddress.equals(macAddress, ignoreCase = true)
        ) {
            lastSelectedMacAddress = macAddress ?: lastSelectedMacAddress
            pendingMacAddress = null
        }
    }

    private fun updateVisibleConnectionStatus() {
        visibleConnectionStatusFlow.value = pendingMacAddress?.let { ConnectionStatus.Connecting(it) }
            ?: deviceServiceConnection.connectionStatusFlow.value
    }

    fun selectDevice(macAddress: String) {
        val current = connectionStatusFlow.value
        // A lost session may be retried; a recovering one already has a retry in flight.
        if ((pendingMacAddress == null && current.isSameSession(macAddress)) ||
            pendingMacAddress.equals(macAddress, ignoreCase = true)
        ) {
            return
        }
        lastSelectedMacAddress = macAddress
        pendingMacAddress = macAddress
        updateVisibleConnectionStatus()
        val intent = intentFactory(application, DeviceService::class.java)
        intent.putExtra(DeviceService.INTENT_EXTRA_MAC_ADDRESS, macAddress)
        try {
            application.startForegroundService(intent)
        } catch (ex: SecurityException) {
            handleServiceStartFailure(ex)
            return
        } catch (ex: IllegalStateException) {
            handleServiceStartFailure(ex)
            return
        }
        bindDeviceService()
    }

    fun onDeviceUnpaired(macAddress: String) {
        val activeAddress = connectionStatusFlow.value.macAddress
        if (activeAddress.equals(macAddress, ignoreCase = true)) stopDeviceService()
        if (lastSelectedMacAddress.equals(macAddress, ignoreCase = true)) lastSelectedMacAddress = null
    }

    fun retryConnection() {
        lastSelectedMacAddress?.let { selectDevice(it) }
    }

    private fun handleServiceStartFailure(ex: Exception) {
        pendingMacAddress = null
        updateVisibleConnectionStatus()
        Log.e(TAG, "failed to start service", ex)
    }

    private fun bindDeviceService() {
        if (isDeviceServiceBound) return
        try {
            isDeviceServiceBound = application.bindService(
                intentFactory(application, DeviceService::class.java),
                deviceServiceConnection,
                0,
            )
            if (!isDeviceServiceBound) pendingMacAddress = null
            updateVisibleConnectionStatus()
        } catch (ex: SecurityException) {
            pendingMacAddress = null
            updateVisibleConnectionStatus()
            Log.e(TAG, "failed to bind service", ex)
        }
    }

    private fun unbindDeviceService() {
        pendingMacAddress = null
        if (isDeviceServiceBound) {
            isDeviceServiceBound = false
            application.unbindService(deviceServiceConnection)
        }
        deviceServiceConnection.close()
        updateVisibleConnectionStatus()
    }

    override fun onCleared() {
        unbindDeviceService()
        deviceSettingsManager.value?.close()
        deviceSettingsManager.value = null
        super.onCleared()
    }

    fun stopDeviceService() {
        pendingMacAddress = null
        // stopService alone leaves the native connection open if a widget screen is also bound.
        val disconnectIntent = intentFactory(DeviceService.ACTION_DISCONNECT)
        disconnectIntent.setPackage(application.packageName)
        application.sendBroadcast(disconnectIntent)
        unbindDeviceService()
        application.stopService(intentFactory(application, DeviceService::class.java))
    }
}
