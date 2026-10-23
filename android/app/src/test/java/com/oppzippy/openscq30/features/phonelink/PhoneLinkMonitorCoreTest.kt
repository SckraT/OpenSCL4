package com.oppzippy.openscq30.features.phonelink

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class PhoneLinkMonitorCoreTest {
    private val liberty = "F4:9D:8A:7C:99:06"
    private val q30 = "AC:12:2F:C8:6E:08"
    private val core = PhoneLinkMonitorCore()

    @Test
    fun profileScansClassicProxiesAndAclEventsAreUnioned() {
        core.applyScan(BluetoothAccess.Ready, setOf(liberty.lowercase()))
        core.onProfileDevices(profile = 1, addresses = setOf(q30))
        core.onAclConnected("00:11:22:33:44:55")

        val signals = core.snapshot.value
        assertEquals(BluetoothAccess.Ready, signals.access)
        assertEquals(setOf("f4:9d:8a:7c:99:06", "ac:12:2f:c8:6e:08"), signals.profileAddresses)
        assertEquals(setOf("00:11:22:33:44:55"), signals.aclAddresses)
    }

    @Test
    fun aclEventsAddAndRemoveLinks() {
        core.applyScan(BluetoothAccess.Ready, emptySet())

        core.onAclConnected(q30)
        assertTrue(q30.lowercase() in core.snapshot.value.aclAddresses)

        core.onAclDisconnected(q30.uppercase())
        assertEquals(emptySet<String>(), core.snapshot.value.aclAddresses)
    }

    @Test
    fun aDisconnectedProfileDropsItsAddresses() {
        core.applyScan(BluetoothAccess.Ready, emptySet())
        core.onProfileDevices(profile = 1, addresses = setOf(liberty))
        core.onProfileDevices(profile = 2, addresses = setOf(q30))

        core.onProfileDisconnected(profile = 1)

        assertEquals(setOf(q30.lowercase()), core.snapshot.value.profileAddresses)
    }

    @Test
    fun turningTheAdapterOffClearsEveryAddressAndTurningItOnStartsEmpty() {
        core.applyScan(BluetoothAccess.Ready, setOf(liberty))
        core.onAclConnected(q30)

        core.onAdapterOff()
        assertEquals(BluetoothAccess.Off, core.snapshot.value.access)
        assertEquals(PhoneLinkSignals(BluetoothAccess.Off), core.snapshot.value)

        core.onAdapterOn()
        assertEquals(BluetoothAccess.Unknown, core.snapshot.value.access)
        assertEquals(PhoneLinkSignals(BluetoothAccess.Unknown), core.snapshot.value)
    }

    @Test
    fun revokingPermissionDropsAllEvidence() {
        core.applyScan(BluetoothAccess.Ready, setOf(liberty))
        core.onAclConnected(q30)

        core.applyScan(BluetoothAccess.PermissionDenied, emptySet())

        assertEquals(PhoneLinkSignals(BluetoothAccess.PermissionDenied), core.snapshot.value)
    }
}
