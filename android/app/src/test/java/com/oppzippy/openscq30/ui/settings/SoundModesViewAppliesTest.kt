package com.oppzippy.openscq30.ui.settings

import com.oppzippy.openscq30.features.soundcoredevice.service.ConnectionStatus
import com.oppzippy.openscq30.features.soundcoredevice.service.DeviceConnectionManager
import com.oppzippy.openscq30.lib.bindings.OpenScq30Device
import com.oppzippy.openscq30.lib.bindings.SessionFailureReason
import com.oppzippy.openscq30.lib.wrapper.Range
import com.oppzippy.openscq30.lib.wrapper.Select
import com.oppzippy.openscq30.lib.wrapper.Setting
import io.mockk.every
import io.mockk.mockk
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SoundModesViewAppliesTest {
    private val modes = listOf("NoiseCanceling", "Transparency", "Normal")
    private val macAddress = "F4:9D:8A:7C:99:06"

    /** A device the scale can show: all three modes and one strength for the current one. */
    private fun scaleDevice(mode: String = "Normal") = mapOf(
        "ambientSoundMode" to Setting.SelectSetting(Select(modes, modes), mode),
        "manualNoiseCanceling" to Setting.I32RangeSetting(Range(1, 5, 1), 3),
        "manualTransparency" to Setting.I32RangeSetting(Range(1, 5, 1), 2),
    )

    /** The connection the device the app controls is reported through. */
    private fun connectionTo(model: String): DeviceConnectionManager {
        val device = mockk<OpenScq30Device>(relaxed = true)
        every { device.model() } returns model
        val connection = mockk<DeviceConnectionManager>(relaxed = true)
        every { connection.device } returns device
        return connection
    }

    private fun connectedTo(model: String) = ConnectionStatus.Connected(connectionTo(model), macAddress)

    @Test
    fun aDeviceShownAsOneScaleHasTwoViewsToChooseBetween() {
        assertTrue(soundModesViewApplies("SoundcoreA3954", scaleDevice()))
    }

    @Test
    fun aDeviceWithoutTheScaleHasNothingToChoose() {
        // Space One Pro offers the modes separately and never the scale, so both views would look the same.
        assertFalse(soundModesViewApplies("SoundcoreA3062", scaleDevice()))
        // the only model with the scale still needs all three modes before the scale can show them
        val noNormal = scaleDevice() + (
            "ambientSoundMode" to Setting.SelectSetting(
                Select(listOf("NoiseCanceling", "Transparency"), listOf("a", "b")),
                "NoiseCanceling",
            )
            )
        assertFalse(soundModesViewApplies("SoundcoreA3954", noNormal))
    }

    @Test
    fun withNothingConnectedThereIsNothingToChoose() {
        assertFalse(soundModesViewApplies(null, emptyMap()))
    }

    /**
     * Both ways into the app settings ask the same question about the device the app controls, so this is also the
     * answer for the settings opened from "My devices" rather than from the device screen.
     */
    @Test
    fun theConnectedDeviceDecidesForEitherWayIntoTheSettings() {
        assertTrue(soundModesViewApplies(connectedTo("SoundcoreA3954"), scaleDevice().toList()))
        // a recovering session keeps its settings, so the choice is still there
        assertTrue(
            soundModesViewApplies(
                ConnectionStatus.Recovering(
                    connectionTo("SoundcoreA3954"),
                    macAddress,
                    attempt = 1,
                    reason = SessionFailureReason.TRANSPORT_LOST,
                ),
                scaleDevice().toList(),
            ),
        )
        assertFalse(soundModesViewApplies(connectedTo("SoundcoreA3062"), scaleDevice().toList()))
    }

    @Test
    fun noDeviceOrNoSettingsYetLeavesNothingToChoose() {
        assertFalse(soundModesViewApplies(ConnectionStatus.Disconnected, scaleDevice().toList()))
        assertFalse(soundModesViewApplies(ConnectionStatus.Connecting(macAddress), scaleDevice().toList()))
        // the settings arrive after the connection, and until they do the scale is unknown
        assertFalse(soundModesViewApplies(connectedTo("SoundcoreA3954"), emptyList()))
    }
}
