package com.oppzippy.openscq30.ui

import android.content.Intent
import android.content.ServiceConnection
import android.util.Log
import androidx.lifecycle.ViewModelStore
import com.oppzippy.openscq30.OpenSCQ30Application
import com.oppzippy.openscq30.android.IntentFactory
import com.oppzippy.openscq30.features.soundcoredevice.service.ConnectionStatus
import com.oppzippy.openscq30.features.soundcoredevice.service.DeviceConnectionManager
import com.oppzippy.openscq30.features.soundcoredevice.service.DeviceService
import com.oppzippy.openscq30.lib.bindings.OpenScq30Device
import com.oppzippy.openscq30.test.MainDispatcherRule
import io.mockk.every
import io.mockk.mockk
import io.mockk.mockkStatic
import io.mockk.slot
import io.mockk.unmockkStatic
import io.mockk.verify
import kotlinx.coroutines.flow.MutableStateFlow
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertSame
import org.junit.Before
import org.junit.Rule
import org.junit.Test

class OpenSCQ30RootViewModelTest {
    @get:Rule
    val mainDispatcherRule = MainDispatcherRule()

    private lateinit var application: OpenSCQ30Application
    private lateinit var intent: Intent
    private lateinit var disconnectIntent: Intent
    private lateinit var viewModel: OpenSCQ30RootViewModel
    private lateinit var store: ViewModelStore
    private val binding = slot<ServiceConnection>()

    @Before
    fun setUp() {
        application = mockk(relaxed = true)
        intent = mockk(relaxed = true)
        disconnectIntent = mockk(relaxed = true)
        val intentFactory = mockk<IntentFactory>()
        every { intentFactory(any(), any()) } returns intent
        every { intentFactory(DeviceService.ACTION_DISCONNECT) } returns disconnectIntent
        every { application.bindService(any(), capture(binding), any<Int>()) } returns true
        viewModel = OpenSCQ30RootViewModel(
            session = mockk(relaxed = true),
            quickPresetSlotDao = mockk(relaxed = true),
            legacyEqualizerProfileDao = mockk(relaxed = true),
            featuredSettingSlotDao = mockk(relaxed = true),
            intentFactory = intentFactory,
            application = application,
            version2BreakingChangesMessage = mockk(),
        )
        store = ViewModelStore().apply { put("root", viewModel) }
    }

    private fun attachService(status: MutableStateFlow<ConnectionStatus>) {
        val service = mockk<DeviceService>()
        val binder = mockk<DeviceService.MyBinder>()
        every { service.connectionStatusFlow } returns status
        every { binder.getService() } returns service
        binding.captured.onServiceConnected(null, binder)
    }

    private fun mockDeviceManager(): DeviceConnectionManager {
        val manager = mockk<DeviceConnectionManager>(relaxed = true)
        every { manager.device } returns mockk<OpenScq30Device>(relaxed = true)
        return manager
    }

    @After
    fun tearDown() {
        store.clear()
    }

    @Test
    fun repeatedSelectionBeforeServiceBindsStartsAndBindsOnlyOnce() {
        viewModel.selectDevice("AA:BB:CC:DD:EE:FF")
        assertEquals(
            "AA:BB:CC:DD:EE:FF",
            (viewModel.connectionStatusFlow.value as ConnectionStatus.Connecting).macAddress,
        )
        viewModel.selectDevice("aa:bb:cc:dd:ee:ff")

        verify(exactly = 1) { application.startForegroundService(any()) }
        verify(exactly = 1) { application.bindService(any(), any(), any<Int>()) }
        store.clear()
    }

    @Test
    fun repeatedSelectionWhileConnectingDoesNotRestartService() {
        viewModel.selectDevice("device")
        attachService(MutableStateFlow(ConnectionStatus.Connecting("device")))
        viewModel.selectDevice("device")

        verify(exactly = 1) { application.startForegroundService(any()) }
        verify(exactly = 1) { application.bindService(any(), any(), any<Int>()) }
        store.clear()
    }

    @Test
    fun switchingDevicesKeepsSingleBinding() {
        viewModel.selectDevice("first")
        attachService(MutableStateFlow(ConnectionStatus.Connecting("first")))
        viewModel.selectDevice("second")

        verify(exactly = 2) { application.startForegroundService(any()) }
        verify(exactly = 1) { application.bindService(any(), any(), any<Int>()) }
        verify { intent.putExtra(DeviceService.INTENT_EXTRA_MAC_ADDRESS, "second") }
        store.clear()
    }

    @Test
    fun staleConnectionStatusDoesNotClearNewSelectionOrChangeRetryAddress() {
        viewModel.selectDevice("first")
        viewModel.selectDevice("second")
        val status = MutableStateFlow<ConnectionStatus>(ConnectionStatus.Connecting("first"))
        attachService(status)
        viewModel.selectDevice("second")
        verify(exactly = 2) { application.startForegroundService(any()) }

        status.value = ConnectionStatus.Disconnected
        viewModel.retryConnection()
        assertEquals("second", (viewModel.connectionStatusFlow.value as ConnectionStatus.Connecting).macAddress)

        verify(exactly = 1) { intent.putExtra(DeviceService.INTENT_EXTRA_MAC_ADDRESS, "first") }
        verify(exactly = 2) { intent.putExtra(DeviceService.INTENT_EXTRA_MAC_ADDRESS, "second") }
        store.clear()
    }

    @Test
    fun recoveringAndLostKeepTheSettingsManagerAndTheBinding() {
        val manager = mockDeviceManager()
        val status = MutableStateFlow<ConnectionStatus>(ConnectionStatus.Connected(manager, "device"))
        viewModel.selectDevice("device")
        attachService(status)
        val settingsManager = viewModel.deviceSettingsManager.value
        assertNotNull(settingsManager)

        status.value = ConnectionStatus.Recovering(manager, "device", attempt = 1, reason = "transport lost")
        assertSame(settingsManager, viewModel.deviceSettingsManager.value)
        assertEquals(
            "device",
            (viewModel.connectionStatusFlow.value as ConnectionStatus.Recovering).macAddress,
        )

        status.value = ConnectionStatus.Lost(manager, "device", "recovery budget exhausted")
        assertSame(settingsManager, viewModel.deviceSettingsManager.value)
        verify(exactly = 0) { application.unbindService(any()) }
        store.clear()
    }

    @Test
    fun selectingTheDeviceWhileRecoveringDoesNotRestartTheService() {
        val manager = mockDeviceManager()
        val status = MutableStateFlow<ConnectionStatus>(ConnectionStatus.Connected(manager, "device"))
        viewModel.selectDevice("device")
        attachService(status)
        status.value = ConnectionStatus.Recovering(manager, "device", attempt = 1, reason = "transport lost")

        viewModel.selectDevice("device")

        verify(exactly = 1) { application.startForegroundService(any()) }
        store.clear()
    }

    @Test
    fun retryFromLostStartsTheServiceAgainForTheSameDevice() {
        val manager = mockDeviceManager()
        viewModel.selectDevice("device")
        attachService(MutableStateFlow(ConnectionStatus.Lost(manager, "device", "recovery budget exhausted")))

        viewModel.retryConnection()

        verify(exactly = 2) { application.startForegroundService(any()) }
        verify(exactly = 2) { intent.putExtra(DeviceService.INTENT_EXTRA_MAC_ADDRESS, "device") }
        store.clear()
    }

    @Test
    fun clearingViewModelUnbindsWithoutStoppingForegroundService() {
        viewModel.selectDevice("device")
        attachService(MutableStateFlow(ConnectionStatus.Connecting("device")))
        store.clear()

        verify(exactly = 1) { application.unbindService(binding.captured) }
        verify(exactly = 0) { application.stopService(any()) }
        assertEquals(ConnectionStatus.Disconnected, viewModel.connectionStatusFlow.value)
    }

    @Test
    fun removingTheActiveDeviceStopsItsServiceAndClearsRetry() {
        viewModel.selectDevice("device")
        viewModel.onDeviceUnpaired("DEVICE")
        viewModel.retryConnection()
        verify(exactly = 1) { application.stopService(any()) }
        verify(exactly = 1) { application.startForegroundService(any()) }
        assertEquals(ConnectionStatus.Disconnected, viewModel.connectionStatusFlow.value)
        store.clear()
    }

    @Test
    fun removingAnotherSavedDevicePreservesTheActiveConnection() {
        viewModel.selectDevice("device")
        viewModel.onDeviceUnpaired("other")
        verify(exactly = 0) { application.stopService(any()) }
        assertEquals("device", (viewModel.connectionStatusFlow.value as ConnectionStatus.Connecting).macAddress)
        store.clear()
    }

    @Test
    fun explicitDisconnectStopsServiceAndUnbindsExactlyOnce() {
        viewModel.selectDevice("device")
        viewModel.stopDeviceService()
        store.clear()

        verify(exactly = 1) { application.stopService(any()) }
        verify(exactly = 1) { disconnectIntent.setPackage(application.packageName) }
        verify(exactly = 1) { application.sendBroadcast(disconnectIntent) }
        verify(exactly = 1) { application.unbindService(binding.captured) }
    }

    @Test
    fun unexpectedServiceDisconnectUnbindsAndRetryUsesLastSelectedAddress() {
        viewModel.selectDevice("device")
        attachService(MutableStateFlow(ConnectionStatus.Connecting("device")))
        binding.captured.onServiceDisconnected(null)

        verify(exactly = 1) { application.unbindService(any()) }
        viewModel.retryConnection()

        verify(exactly = 2) { application.startForegroundService(any()) }
        verify(exactly = 2) { application.bindService(any(), any(), any<Int>()) }
        verify(exactly = 2) { intent.putExtra(DeviceService.INTENT_EXTRA_MAC_ADDRESS, "device") }
        store.clear()
        verify(exactly = 2) { application.unbindService(any()) }
    }

    @Test
    fun retryAfterConnectionFailureUsesLastSelectedAddress() {
        viewModel.selectDevice("device")
        val status = MutableStateFlow<ConnectionStatus>(ConnectionStatus.Connecting("device"))
        attachService(status)
        status.value = ConnectionStatus.Disconnected
        viewModel.retryConnection()

        verify(exactly = 2) { application.startForegroundService(any()) }
        verify(exactly = 2) { intent.putExtra(DeviceService.INTENT_EXTRA_MAC_ADDRESS, "device") }
        verify(exactly = 2) { application.bindService(any(), any(), any<Int>()) }
        verify(exactly = 1) { application.unbindService(any()) }
        store.clear()
    }

    @Test
    fun failedBindDoesNotGetUnboundAndAllowsRetry() {
        every { application.bindService(any(), capture(binding), any<Int>()) } returns false
        viewModel.selectDevice("device")
        assertEquals(ConnectionStatus.Disconnected, viewModel.connectionStatusFlow.value)
        viewModel.retryConnection()
        store.clear()

        verify(exactly = 2) { application.bindService(any(), any(), any<Int>()) }
        verify(exactly = 0) { application.unbindService(any()) }
        verify(exactly = 0) { application.stopService(any()) }
    }

    @Test
    fun deniedForegroundServiceStartReturnsToDisconnectedAndCanRetry() {
        mockkStatic(Log::class)
        try {
            every { Log.e(any(), any(), any()) } returns 0
            every { application.startForegroundService(any()) } throws SecurityException("permission denied")

            viewModel.selectDevice("device")
            assertEquals(ConnectionStatus.Disconnected, viewModel.connectionStatusFlow.value)

            every { application.startForegroundService(any()) } returns null
            viewModel.retryConnection()
            assertEquals("device", (viewModel.connectionStatusFlow.value as ConnectionStatus.Connecting).macAddress)
            verify(exactly = 2) { application.startForegroundService(any()) }
        } finally {
            unmockkStatic(Log::class)
        }
    }
}
