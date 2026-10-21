package com.oppzippy.openscq30.ui.deviceselection

import com.oppzippy.openscq30.features.phonelink.BluetoothAccess
import com.oppzippy.openscq30.features.phonelink.PhoneLinkSignals
import com.oppzippy.openscq30.lib.wrapper.PairedDevice
import org.junit.Assert.assertEquals
import org.junit.Test

class FeaturedDevicePinnerTest {
    private val liberty = PairedDevice("F4:9D:8A:7C:99:06", "SoundcoreA3954", false)
    private val q30 = PairedDevice("AC:12:2F:C8:6E:08", "SoundcoreA3028", false)
    private val devices = listOf(q30, liberty)

    @Test
    fun theFeaturedDeviceDoesNotJumpWhenThePhoneLinkChanges() {
        val pinner = FeaturedDevicePinner()
        val control = AppControlSnapshot()

        assertEquals(liberty, pin(pinner, linked = setOf(liberty.macAddress), control, q30.macAddress))
        assertEquals(liberty, pin(pinner, linked = emptySet(), control, q30.macAddress))
    }

    @Test
    fun takingControlMovesTheFeaturedDevice() {
        val pinner = FeaturedDevicePinner()

        pin(pinner, linked = emptySet(), AppControlSnapshot(), q30.macAddress)
        assertEquals(
            liberty,
            pin(
                pinner,
                linked = emptySet(),
                AppControlSnapshot(liberty.macAddress, AppControl.Controlling),
                q30.macAddress,
            ),
        )
    }

    @Test
    fun usingAnotherDeviceMovesTheFeaturedDevice() {
        val pinner = FeaturedDevicePinner()

        pin(pinner, linked = setOf(liberty.macAddress), AppControlSnapshot(), null)
        assertEquals(liberty, pin(pinner, linked = setOf(liberty.macAddress), AppControlSnapshot(), q30.macAddress))
    }

    @Test
    fun reopeningTheListPicksTheCurrentLinkAgain() {
        val pinner = FeaturedDevicePinner()

        assertEquals(liberty, pin(pinner, linked = setOf(liberty.macAddress), AppControlSnapshot(), null))
        pinner.reset()
        assertEquals(q30, pin(pinner, linked = emptySet(), AppControlSnapshot(), null))
    }

    private fun pin(
        pinner: FeaturedDevicePinner,
        linked: Set<String>,
        control: AppControlSnapshot,
        lastUsed: String?,
    ): PairedDevice? {
        val pinned = pinner.pinnedFeaturedMacAddress(devices, control, lastUsed)
        val state = DeviceListPresenter.present(
            devices = devices,
            phoneLink = PhoneLinkSignals(
                access = BluetoothAccess.Ready,
                profileAddresses = linked,
            ),
            appControl = control,
            lastDeviceMacAddress = lastUsed,
            featuredMacAddress = pinned,
        )
        pinner.remember(state, devices, control, lastUsed)
        return state.featured?.device
    }
}
