package com.oppzippy.openscq30.widget

import com.oppzippy.openscq30.lib.bindings.AvailabilityReason
import com.oppzippy.openscq30.lib.wrapper.Select
import com.oppzippy.openscq30.lib.wrapper.Setting
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class SettingWidgetSettingsTest {
    private fun select(vararg options: String, value: String): Setting = Setting.SelectSetting(
        Select(options.toList(), options.toList()),
        value,
    )

    private val musicMode = Setting.ToggleSetting(true)

    private fun rowsOf(
        selected: List<Pair<String, Setting?>>,
        values: Map<String, Setting>,
    ): List<WidgetSetting> = widgetSettings(selected, values)

    private fun spatialAudio(musicMode: Boolean, mode: String): Map<String, Setting> = mapOf(
        "spatialAudio" to Setting.ToggleSetting(true),
        "spatialAudioMusicMode" to Setting.ToggleSetting(musicMode),
        "spatialAudioMode" to select("Music", "Podcast", value = mode),
    )

    @Test
    fun theMusicModeIsHiddenWhileSpatialAudioIsOff() {
        val values = mapOf(
            "spatialAudio" to Setting.ToggleSetting(false),
            "spatialAudioMode" to select("Music", "Podcast", value = "Music"),
            "spatialAudioMusicMode" to musicMode,
        )

        assertTrue(widgetSettings(listOf("spatialAudioMusicMode" to musicMode), values).isEmpty())
    }

    @Test
    fun theMusicModeIsEditableWhileSpatialAudioIsOn() {
        assertEquals(
            listOf(WidgetSetting.Editable("spatialAudioMusicMode", musicMode)),
            rowsOf(listOf("spatialAudioMusicMode" to musicMode), spatialAudio(musicMode = true, mode = "Music")),
        )
    }

    @Test
    fun theMusicModeIsHiddenWhenTheAudioModeIsNotMusic() {
        assertTrue(
            rowsOf(listOf("spatialAudioMusicMode" to musicMode), spatialAudio(musicMode = true, mode = "Podcast"))
                .isEmpty(),
        )
    }

    @Test
    fun availabilityWithOneEarbudKeepsTheValueAndItsReason() {
        val settings = mapOf(
            "twsStatus" to Setting.InformationSetting("Disconnected", "Disconnected"),
            "ldac" to Setting.ToggleSetting(true),
        )

        assertEquals(
            listOf(
                WidgetSetting.ReadOnly(
                    "ldac",
                    Setting.ToggleSetting(true),
                    AvailabilityReason.REQUIRES_BOTH_EARBUDS,
                ),
            ),
            rowsOf(listOf("ldac" to Setting.ToggleSetting(true)), settings),
        )
    }

    @Test
    fun anUnavailableSettingShowsOnlyTheReason() {
        val settings = mapOf(
            "twsStatus" to Setting.InformationSetting("Disconnected", "Disconnected"),
            "volumeAdjustments" to Setting.ToggleSetting(true),
        )

        assertEquals(
            listOf(WidgetSetting.Blocked("volumeAdjustments", AvailabilityReason.REQUIRES_BOTH_EARBUDS)),
            rowsOf(listOf("volumeAdjustments" to Setting.ToggleSetting(true)), settings),
        )
    }

    @Test
    fun availabilityOutsideTheDeviceScreenBlocksTheSetting() {
        // the widget may be drawn before a snapshot provides availability: the setting must not look changeable
        assertEquals(
            listOf(WidgetSetting.Blocked("spatialAudioMusicMode", AvailabilityReason.NO_DEVICE_STATE)),
            widgetSettings(listOf("spatialAudioMusicMode" to musicMode), emptyMap()),
        )
    }

    @Test
    fun availableSettingsKeepTheirControl() {
        val settings = mapOf("wearingDetection" to Setting.ToggleSetting(true))

        assertEquals(
            listOf(WidgetSetting.Editable("wearingDetection", Setting.ToggleSetting(true))),
            rowsOf(listOf("wearingDetection" to Setting.ToggleSetting(true)), settings),
        )
    }

    @Test
    fun settingsTheDeviceDoesNotReportAreSkipped() {
        assertTrue(rowsOf(listOf("somethingNew" to null), spatialAudio(musicMode = true, mode = "Music")).isEmpty())
    }
}
