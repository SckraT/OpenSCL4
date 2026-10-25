package com.oppzippy.openscq30.features.autoconnect

import com.oppzippy.openscq30.lib.wrapper.PairedDevice
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class AutoConnectCoordinatorTest {
    private val dispatched = mutableListOf<String>()

    private fun coordinator(
        autoConnect: Boolean = true,
        associated: List<String> = listOf("AA:BB:CC:DD:EE:FF"),
        saved: List<PairedDevice> = listOf(PairedDevice("AA:BB:CC:DD:EE:FF", "model", false)),
    ) = AutoConnectCoordinator(
        isAutoConnectEnabled = { autoConnect },
        associatedAddresses = { associated },
        savedDevices = { saved },
        connect = { dispatched.add(it) },
    )

    @Test
    fun reconcileConnectsAlreadyConnectedAssociatedSavedDevice() = runTest {
        coordinator().reconcileConnectedDevices(listOf("AA:BB:CC:DD:EE:FF"))

        assertEquals(listOf("AA:BB:CC:DD:EE:FF"), dispatched)
    }

    @Test
    fun reconcileMatchesAddressesCaseInsensitively() = runTest {
        coordinator(
            associated = listOf("aa:bb:cc:dd:ee:ff"),
            saved = listOf(PairedDevice("Aa:Bb:Cc:Dd:Ee:Ff", "model", false)),
        ).reconcileConnectedDevices(listOf("AA:BB:CC:DD:EE:FF"))

        assertEquals(listOf("AA:BB:CC:DD:EE:FF"), dispatched)
    }

    @Test
    fun reconcileIgnoresDeviceThatIsNotAssociated() = runTest {
        coordinator(associated = listOf("11:22:33:44:55:66"))
            .reconcileConnectedDevices(listOf("AA:BB:CC:DD:EE:FF"))

        assertEquals(emptyList<String>(), dispatched)
    }

    @Test
    fun reconcileIgnoresDeviceThatIsNotSaved() = runTest {
        coordinator(saved = listOf(PairedDevice("11:22:33:44:55:66", "other", false)))
            .reconcileConnectedDevices(listOf("AA:BB:CC:DD:EE:FF"))

        assertEquals(emptyList<String>(), dispatched)
    }

    @Test
    fun reconcileIgnoresDemoDevices() = runTest {
        coordinator(saved = listOf(PairedDevice("AA:BB:CC:DD:EE:FF", "demo", true)))
            .reconcileConnectedDevices(listOf("AA:BB:CC:DD:EE:FF"))

        assertEquals(emptyList<String>(), dispatched)
    }

    @Test
    fun reconcileDeduplicatesSameDeviceCaseInsensitively() = runTest {
        coordinator().reconcileConnectedDevices(listOf("AA:BB:CC:DD:EE:FF", "aa:bb:cc:dd:ee:ff"))

        assertEquals(listOf("AA:BB:CC:DD:EE:FF"), dispatched)
    }

    @Test
    fun reconcileDoesNothingWhenAutoConnectIsDisabled() = runTest {
        coordinator(autoConnect = false).reconcileConnectedDevices(listOf("AA:BB:CC:DD:EE:FF"))

        assertEquals(emptyList<String>(), dispatched)
    }

    @Test
    fun reconcileDoesNothingWhenNoDevicesAreConnected() = runTest {
        coordinator().reconcileConnectedDevices(emptyList())

        assertEquals(emptyList<String>(), dispatched)
    }

    @Test
    fun reconcileStopsWhenAutoConnectIsDisabledWhileLoadingDevices() = runTest {
        var autoConnect = true
        val coordinator = AutoConnectCoordinator(
            isAutoConnectEnabled = { autoConnect },
            associatedAddresses = { listOf("AA:BB:CC:DD:EE:FF") },
            savedDevices = {
                autoConnect = false
                listOf(PairedDevice("AA:BB:CC:DD:EE:FF", "model", false))
            },
            connect = { dispatched.add(it) },
        )

        coordinator.reconcileConnectedDevices(listOf("AA:BB:CC:DD:EE:FF"))

        assertEquals(emptyList<String>(), dispatched)
    }

    @Test
    fun broadcastUsesTheSameEligibilityRuleAsReconciliation() = runTest {
        coordinator().handleConnectedDevice("AA:BB:CC:DD:EE:FF")

        assertEquals(listOf("AA:BB:CC:DD:EE:FF"), dispatched)
    }

    @Test
    fun broadcastIgnoresUnwantedDevices() = runTest {
        coordinator(saved = listOf(PairedDevice("AA:BB:CC:DD:EE:FF", "demo", true)))
            .handleConnectedDevice("AA:BB:CC:DD:EE:FF")

        assertEquals(emptyList<String>(), dispatched)
    }

    @Test
    fun broadcastDoesNothingWhenAutoConnectIsDisabled() = runTest {
        coordinator(autoConnect = false).handleConnectedDevice("AA:BB:CC:DD:EE:FF")

        assertEquals(emptyList<String>(), dispatched)
    }
}
