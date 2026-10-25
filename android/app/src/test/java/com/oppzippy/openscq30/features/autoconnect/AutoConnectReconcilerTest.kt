package com.oppzippy.openscq30.features.autoconnect

import android.bluetooth.BluetoothProfile
import com.oppzippy.openscq30.android.FakeBluetoothProfiles
import com.oppzippy.openscq30.lib.wrapper.PairedDevice
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class AutoConnectReconcilerTest {
    private val liberty = "F4:9D:8A:7C:99:06"
    private val bluetooth = FakeBluetoothProfiles()
    private val dispatched = mutableListOf<String>()

    private fun coordinator() = AutoConnectCoordinator(
        isAutoConnectEnabled = { true },
        associatedAddresses = { listOf(liberty) },
        savedDevices = { listOf(PairedDevice(liberty, "model", false)) },
        connect = { dispatched.add(it) },
    )

    private fun TestScope.reconciler(coordinator: AutoConnectCoordinator = coordinator()) = AutoConnectReconciler(
        bluetooth = bluetooth,
        coordinator = coordinator,
        scope = this,
    )

    @Test
    fun startConnectsADeviceThatOnlyAClassicProfileReports() = runTest {
        bluetooth.devices[BluetoothProfile.A2DP] = listOf(liberty)

        reconciler().start()
        bluetooth.connectProxies()
        advanceUntilIdle()

        assertEquals(listOf(liberty), dispatched)
    }

    @Test
    fun startConnectsADeviceOnceWhenSeveralProfilesReportIt() = runTest {
        bluetooth.devices[BluetoothProfile.A2DP] = listOf(liberty)
        bluetooth.devices[BluetoothProfile.HEADSET] = listOf(liberty)
        bluetooth.devices[BluetoothProfile.GATT] = listOf(liberty)

        reconciler().start()
        bluetooth.connectProxies()
        advanceUntilIdle()

        assertEquals(listOf(liberty), dispatched)
    }

    @Test
    fun aDeviceTheBroadcastStartedBeforeItsProxyReportedIsStartedOnce() = runTest {
        bluetooth.devices[BluetoothProfile.A2DP] = listOf(liberty)
        val coordinator = coordinator()
        reconciler(coordinator).start()

        coordinator.handleConnectedDevice(liberty)
        bluetooth.connectProxies()
        advanceUntilIdle()

        assertEquals(listOf(liberty), dispatched)
    }

    @Test
    fun stopClosesTheClassicProfileProxies() = runTest {
        val reconciler = reconciler()
        reconciler.start()
        bluetooth.connectProxies()

        reconciler.stop()

        assertEquals(
            listOf(BluetoothProfile.A2DP, BluetoothProfile.HEADSET),
            bluetooth.proxies.map { it.request.profile },
        )
        assertEquals(emptyList<FakeBluetoothProfiles.FakeProxy>(), bluetooth.openProxies)
    }

    @Test
    fun restartingBeforeTheStoppedProxiesConnectStartsTheDeviceOnceAndClosesThem() = runTest {
        bluetooth.devices[BluetoothProfile.A2DP] = listOf(liberty)
        val stopped = reconciler()
        stopped.start()
        stopped.stop()

        reconciler().start()
        bluetooth.connectProxies()
        advanceUntilIdle()

        assertEquals(listOf(liberty), dispatched)
        assertEquals(
            listOf(BluetoothProfile.A2DP, BluetoothProfile.HEADSET),
            bluetooth.openProxies.map { it.request.profile },
        )
    }
}
