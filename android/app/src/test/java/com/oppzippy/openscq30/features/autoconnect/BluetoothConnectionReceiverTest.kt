package com.oppzippy.openscq30.features.autoconnect

import android.bluetooth.BluetoothA2dp
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothHeadset
import android.bluetooth.BluetoothProfile
import android.content.Intent
import androidx.core.content.IntentCompat
import com.oppzippy.openscq30.lib.wrapper.PairedDevice
import io.mockk.every
import io.mockk.mockk
import io.mockk.mockkStatic
import io.mockk.unmockkStatic
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class BluetoothConnectionReceiverTest {
    private val address = "AA:BB:CC:DD:EE:FF"
    private val dispatched = mutableListOf<String>()
    private val coordinator = AutoConnectCoordinator(
        isAutoConnectEnabled = { true },
        associatedAddresses = { listOf(address) },
        savedDevices = { listOf(PairedDevice(address, "model", false)) },
        connect = { dispatched.add(it) },
    )
    private val receiver = BluetoothConnectionReceiver(CoroutineScope(UnconfinedTestDispatcher()), coordinator)

    @Before
    fun setUp() {
        val device = mockk<BluetoothDevice>()
        every { device.address } returns address
        mockkStatic(IntentCompat::class)
        every {
            IntentCompat.getParcelableExtra(any<Intent>(), any<String>(), any<Class<BluetoothDevice>>())
        } returns device
    }

    @After
    fun tearDown() {
        unmockkStatic(IntentCompat::class)
    }

    private fun broadcast(action: String, state: Int = BluetoothProfile.STATE_CONNECTED) {
        val intent = mockk<Intent>()
        every { intent.action } returns action
        every { intent.getIntExtra(BluetoothProfile.EXTRA_STATE, any()) } returns state
        receiver.onReceive(null, intent)
    }

    @Test
    fun aProfileThatConnectsAfterTheStartReconcileConnectsTheDevice() = runTest {
        // Started by a presence event: the ACL broadcast went out before this receiver was registered, and the start
        // reconcile read the profiles before A2DP came up.
        coordinator.reconcileConnectedDevices(emptyList())

        broadcast(BluetoothA2dp.ACTION_CONNECTION_STATE_CHANGED)

        assertEquals(listOf(address), dispatched)
    }

    @Test
    fun aHeadsetProfileThatConnectsLaterConnectsTheDeviceToo() {
        broadcast(BluetoothHeadset.ACTION_CONNECTION_STATE_CHANGED)

        assertEquals(listOf(address), dispatched)
    }

    @Test
    fun bothProfilesConnectingStartTheDeviceOnce() {
        broadcast(BluetoothHeadset.ACTION_CONNECTION_STATE_CHANGED)
        broadcast(BluetoothA2dp.ACTION_CONNECTION_STATE_CHANGED)

        assertEquals(listOf(address), dispatched)
    }

    @Test
    fun theProfilesOfADeviceItsAclBroadcastStartedDoNotStartItAgain() {
        broadcast(BluetoothDevice.ACTION_ACL_CONNECTED)
        broadcast(BluetoothA2dp.ACTION_CONNECTION_STATE_CHANGED)
        broadcast(BluetoothHeadset.ACTION_CONNECTION_STATE_CHANGED)

        assertEquals(listOf(address), dispatched)
    }

    @Test
    fun aProfileThatIsNotConnectedIsIgnored() {
        for (state in listOf(
            BluetoothProfile.STATE_CONNECTING,
            BluetoothProfile.STATE_DISCONNECTING,
            BluetoothProfile.STATE_DISCONNECTED,
        )) {
            broadcast(BluetoothA2dp.ACTION_CONNECTION_STATE_CHANGED, state)
        }

        assertEquals(emptyList<String>(), dispatched)
    }

    @Test
    fun otherBroadcastsAreIgnored() {
        broadcast(BluetoothDevice.ACTION_ACL_DISCONNECTED)

        assertEquals(emptyList<String>(), dispatched)
    }
}
