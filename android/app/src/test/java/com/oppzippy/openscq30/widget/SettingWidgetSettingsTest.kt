package com.oppzippy.openscq30.widget

import com.oppzippy.openscq30.lib.bindings.AvailabilityReason
import com.oppzippy.openscq30.lib.wrapper.Select
import com.oppzippy.openscq30.lib.wrapper.Setting
import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class SettingWidgetSettingsTest {
    private fun select(vararg options: String, value: String): Setting = Setting.SelectSetting(
        Select(options.toList(), options.toList()),
        value,
    )

    private val musicMode = Setting.ToggleSetting(true)

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

        assertTrue(widgetSettings(listOf("spatialAudioMusicMode"), values).isEmpty())
    }

    @Test
    fun theMusicModeIsEditableWhileSpatialAudioIsOn() {
        assertEquals(
            listOf(WidgetSetting.Editable("spatialAudioMusicMode", musicMode)),
            widgetSettings(listOf("spatialAudioMusicMode"), spatialAudio(musicMode = true, mode = "Music")),
        )
    }

    @Test
    fun theMusicModeIsHiddenWhenTheAudioModeIsNotMusic() {
        assertTrue(
            widgetSettings(listOf("spatialAudioMusicMode"), spatialAudio(musicMode = true, mode = "Podcast"))
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
            widgetSettings(listOf("ldac"), settings),
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
            widgetSettings(listOf("volumeAdjustments"), settings),
        )
    }

    @Test
    fun aCachedSnapshotFromAnOlderVersionOffersNoControls() {
        val oldState = """
            {
              "type": "com.oppzippy.openscq30.widget.SettingWidgetState.Connected",
              "deviceName": "Liberty 4 Pro",
              "settings": [],
              "snapshot": []
            }
        """.trimIndent()

        assertEquals(
            SettingWidgetState.Connected("Liberty 4 Pro"),
            Json.decodeFromString<SettingWidgetState>(oldState),
        )
    }

    @Test
    fun availableSettingsKeepTheirControl() {
        val settings = mapOf("wearingDetection" to Setting.ToggleSetting(true))

        assertEquals(
            listOf(WidgetSetting.Editable("wearingDetection", Setting.ToggleSetting(true))),
            widgetSettings(listOf("wearingDetection"), settings),
        )
    }

    @Test
    fun settingsTheDeviceDoesNotReportAreSkipped() {
        assertTrue(widgetSettings(listOf("somethingNew"), spatialAudio(musicMode = true, mode = "Music")).isEmpty())
    }

    @Test
    fun cachedRowsKeepAvailabilityAndIgnoreUnrelatedSettings() {
        val snapshot = mapOf(
            "twsStatus" to Setting.InformationSetting("Disconnected", "Disconnected"),
            "ldac" to Setting.ToggleSetting(true),
            "wearingDetection" to Setting.ToggleSetting(true),
        )
        val rows = widgetSettings(listOf("ldac"), snapshot)
        val changed = snapshot + ("wearingDetection" to Setting.ToggleSetting(false))
        assertEquals(rows, widgetSettings(listOf("ldac"), changed))

        val state: SettingWidgetState = SettingWidgetState.Connected("Liberty 4 Pro", rows)
        assertEquals(state, Json.decodeFromString<SettingWidgetState>(Json.encodeToString(state)))
    }
}
