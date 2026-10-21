package com.oppzippy.openscq30.ui.deviceselection

import com.oppzippy.openscq30.features.phonelink.BluetoothAccess
import com.oppzippy.openscq30.features.phonelink.PhoneBluetoothLink
import com.oppzippy.openscq30.features.phonelink.PhoneLinkSignals
import com.oppzippy.openscq30.lib.wrapper.PairedDevice
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class DeviceListPresenterTest {
    private val liberty = PairedDevice("F4:9D:8A:7C:99:06", "SoundcoreA3954", false)
    private val q30 = PairedDevice("AC:12:2F:C8:6E:08", "SoundcoreA3028", false)
    private val savedDevices = listOf(q30, liberty)

    @Test
    fun bothAxesSurviveTheFourLinkAndControlCombinations() {
        val linked = setOf(liberty.macAddress)

        val linkedControlled = present(
            profile = linked,
            control = AppControl.Controlling,
            controlMac = liberty.macAddress,
            lastUsed = liberty.macAddress,
        ).featured!!
        assertEquals(PhoneBluetoothLink.Linked, linkedControlled.link)
        assertEquals(AppControl.Controlling, linkedControlled.control)
        assertEquals(DeviceRowStatus.Connected, linkedControlled.status)
        assertEquals(LinkBadge.Linked, linkedControlled.linkBadge)
        assertEquals(ControlBadge.Active, linkedControlled.controlBadge)
        assertEquals(TapAction.Open, linkedControlled.tapAction)

        val linkedIdle = present(profile = linked, lastUsed = liberty.macAddress).featured!!
        assertEquals(PhoneBluetoothLink.Linked, linkedIdle.link)
        assertEquals(AppControl.NotControlling, linkedIdle.control)
        assertEquals(DeviceRowStatus.LinkedWithoutControl, linkedIdle.status)
        assertEquals(TapAction.Connect, linkedIdle.tapAction)

        val unlinkedIdle = present(lastUsed = liberty.macAddress).featured!!
        assertEquals(PhoneBluetoothLink.NotLinked, unlinkedIdle.link)
        assertEquals(AppControl.NotControlling, unlinkedIdle.control)
        assertEquals(DeviceRowStatus.NotConnected, unlinkedIdle.status)
        assertEquals(LinkBadge.NotLinked, unlinkedIdle.linkBadge)
        assertEquals(TapAction.Connect, unlinkedIdle.tapAction)

        val unlinkedControlled = present(
            control = AppControl.Controlling,
            controlMac = liberty.macAddress,
            lastUsed = liberty.macAddress,
        ).featured!!
        assertEquals(PhoneBluetoothLink.Unknown, unlinkedControlled.link)
        assertEquals(AppControl.Controlling, unlinkedControlled.control)
        assertEquals(DeviceRowStatus.Connected, unlinkedControlled.status)
        assertEquals(LinkBadge.Unconfirmed, unlinkedControlled.linkBadge)
        assertEquals(TapAction.Open, unlinkedControlled.tapAction)
    }

    @Test
    fun aLinkedRowWithoutControlShowsTheDivergence() {
        val row = present(
            profile = setOf(liberty.macAddress),
            control = AppControl.Controlling,
            controlMac = q30.macAddress,
            lastUsed = q30.macAddress,
        ).others.single()

        assertEquals(liberty, row.device)
        assertEquals(PhoneBluetoothLink.Linked, row.link)
        assertEquals(AppControl.NotControlling, row.control)
        assertEquals(DeviceRowStatus.LinkedWithoutControl, row.status)
        assertEquals(TapAction.Connect, row.tapAction)
    }

    @Test
    fun connectingDisablesTheTapAction() {
        val row = present(
            profile = setOf(liberty.macAddress),
            control = AppControl.Connecting,
            controlMac = liberty.macAddress,
            lastUsed = liberty.macAddress,
        ).featured!!

        assertEquals(DeviceRowStatus.Connecting, row.status)
        assertEquals(ControlBadge.Connecting, row.controlBadge)
        assertEquals(TapAction.None, row.tapAction)
    }

    @Test
    fun multipointShowsOtherLinkedDevicesWithoutControl() {
        val state = present(
            profile = setOf(liberty.macAddress, q30.macAddress),
            control = AppControl.Controlling,
            controlMac = q30.macAddress,
            lastUsed = liberty.macAddress,
        )

        assertEquals(q30, state.featured?.device)
        assertEquals(DeviceRowStatus.Connected, state.featured?.status)

        val other = state.others.single()
        assertEquals(liberty, other.device)
        assertEquals(PhoneBluetoothLink.Linked, other.link)
        assertEquals(AppControl.NotControlling, other.control)
        assertEquals(DeviceRowStatus.LinkedWithoutControl, other.status)
        assertEquals(TapAction.Connect, other.tapAction)
    }

    @Test
    fun macAddressesAreComparedCaseInsensitively() {
        val state = present(
            profile = setOf(liberty.macAddress.lowercase()),
            control = AppControl.Controlling,
            controlMac = liberty.macAddress.lowercase(),
            lastUsed = liberty.macAddress.lowercase(),
        )

        assertEquals(liberty, state.featured?.device)
        assertEquals(PhoneBluetoothLink.Linked, state.featured?.link)
        assertEquals(AppControl.Controlling, state.featured?.control)
    }

    @Test
    fun bluetoothAccessOtherThanReadyNeverRendersNotLinked() {
        val inaccessible = listOf(
            BluetoothAccess.Off,
            BluetoothAccess.PermissionDenied,
            BluetoothAccess.Unsupported,
            BluetoothAccess.Unknown,
        )

        for (access in inaccessible) {
            val state = present(profile = setOf(liberty.macAddress), access = access, lastUsed = liberty.macAddress)
            val rows = listOfNotNull(state.featured) + state.others

            assertEquals(access, state.access)
            assertTrue(rows.all { it.link == PhoneBluetoothLink.Unknown })
            assertTrue(rows.all { it.status == DeviceRowStatus.BluetoothUnknown })
            assertTrue(rows.all { it.tapAction == TapAction.ResolveBluetoothAccess })
        }
    }

    @Test
    fun activeControlStaysVisibleWhenBluetoothCannotBeObserved() {
        val row = present(
            access = BluetoothAccess.Off,
            control = AppControl.Controlling,
            controlMac = liberty.macAddress,
            lastUsed = liberty.macAddress,
        ).featured!!

        assertEquals(PhoneBluetoothLink.Unknown, row.link)
        assertEquals(DeviceRowStatus.Connected, row.status)
        assertEquals(LinkBadge.Unconfirmed, row.linkBadge)
        assertEquals(TapAction.Open, row.tapAction)
    }

    @Test
    fun linkChangesUpdateRowsWhileTheListIsOpen() {
        val linked = present(profile = setOf(liberty.macAddress), lastUsed = liberty.macAddress)
        assertEquals(DeviceRowStatus.LinkedWithoutControl, linked.featured?.status)

        val unlinked = present(lastUsed = liberty.macAddress)
        assertEquals(liberty, unlinked.featured?.device)
        assertEquals(PhoneBluetoothLink.NotLinked, unlinked.featured?.link)
        assertEquals(AppControl.NotControlling, unlinked.featured?.control)
        assertEquals(DeviceRowStatus.NotConnected, unlinked.featured?.status)
    }

    @Test
    fun controlLostWhileTheLinkRemainsShowsLinkedWithoutControl() {
        val controlled = present(
            profile = setOf(liberty.macAddress),
            control = AppControl.Controlling,
            controlMac = liberty.macAddress,
            lastUsed = liberty.macAddress,
        )
        assertEquals(DeviceRowStatus.Connected, controlled.featured?.status)

        val withoutControl = present(profile = setOf(liberty.macAddress), lastUsed = liberty.macAddress)
        assertEquals(PhoneBluetoothLink.Linked, withoutControl.featured?.link)
        assertEquals(AppControl.NotControlling, withoutControl.featured?.control)
        assertEquals(DeviceRowStatus.LinkedWithoutControl, withoutControl.featured?.status)
        assertEquals(TapAction.Connect, withoutControl.featured?.tapAction)
    }

    @Test
    fun anActiveSessionWithoutAnObservedLinkIsNotReportedAsNotLinked() {
        val row = present(
            control = AppControl.Controlling,
            controlMac = liberty.macAddress,
            lastUsed = liberty.macAddress,
        ).featured!!

        assertEquals(AppControl.Controlling, row.control)
        assertEquals(PhoneBluetoothLink.Unknown, row.link)
        assertNotEquals(PhoneBluetoothLink.NotLinked, row.link)
        assertEquals(DeviceRowStatus.Connected, row.status)
        assertEquals(LinkBadge.Unconfirmed, row.linkBadge)
    }

    @Test
    fun aDemoDeviceKeepsItsOwnStateAndIgnoresRealLinks() {
        val demo = PairedDevice("F4:9D:8A:7C:99:06", "SoundcoreA3954", true)
        val row = present(
            devices = listOf(demo),
            profile = setOf(demo.macAddress),
            control = AppControl.Controlling,
            controlMac = demo.macAddress,
        ).featured!!

        assertEquals(DeviceRowStatus.Demo, row.status)
        assertEquals(PhoneBluetoothLink.Unknown, row.link)
        assertEquals(AppControl.NotControlling, row.control)
        assertEquals(TapAction.Open, row.tapAction)
    }

    @Test
    fun featuredOrderIsControllingThenLinkedThenLastUsedThenFirstSaved() {
        assertEquals(
            liberty,
            present(
                profile = setOf(q30.macAddress),
                control = AppControl.Controlling,
                controlMac = liberty.macAddress,
                lastUsed = q30.macAddress,
            ).featured?.device,
        )
        assertEquals(q30, present(profile = setOf(q30.macAddress), lastUsed = liberty.macAddress).featured?.device)
        assertEquals(
            liberty,
            present(
                profile = setOf(q30.macAddress, liberty.macAddress),
                lastUsed = liberty.macAddress,
            ).featured?.device,
        )
        assertEquals(liberty, present(lastUsed = liberty.macAddress).featured?.device)
        assertEquals(q30, present().featured?.device)
    }

    @Test
    fun othersKeepTheirSavedOrder() {
        val third = PairedDevice("11:22:33:44:55:66", "SoundcoreA3000", false)
        val state = present(devices = listOf(q30, liberty, third), profile = setOf(q30.macAddress))

        assertEquals(q30, state.featured?.device)
        assertEquals(listOf(liberty, third), state.others.map { it.device })
    }

    private fun present(
        devices: List<PairedDevice> = savedDevices,
        profile: Set<String> = emptySet(),
        acl: Set<String> = emptySet(),
        access: BluetoothAccess = BluetoothAccess.Ready,
        control: AppControl = AppControl.NotControlling,
        controlMac: String? = null,
        lastUsed: String? = null,
    ): DeviceListState = DeviceListPresenter.present(
        devices = devices,
        phoneLink = PhoneLinkSignals(access = access, profileAddresses = profile, aclAddresses = acl),
        appControl = AppControlSnapshot(macAddress = controlMac, phase = control),
        lastDeviceMacAddress = lastUsed,
    )
}
