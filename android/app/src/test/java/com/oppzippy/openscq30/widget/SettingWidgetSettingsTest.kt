package com.oppzippy.openscq30.widget

import com.oppzippy.openscq30.lib.bindings.AvailabilityReason
import com.oppzippy.openscq30.lib.wrapper.Setting
import com.oppzippy.openscq30.ui.sound.SettingGroup
import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SettingWidgetSettingsTest {
    private val musicMode = Setting.ToggleSetting(true)

    private fun spatialAudio(musicMode: Boolean, mode: String): Map<String, Setting> = mapOf(
        "spatialAudio" to Setting.ToggleSetting(true),
        "spatialAudioMusicMode" to Setting.ToggleSetting(musicMode),
        "spatialAudioMode" to selectSetting("Music", "Podcast", value = mode),
    )

    private val spatialAudioCategories = settingCategories(
        "miscellaneous" to listOf("spatialAudio", "spatialAudioMode", "spatialAudioMusicMode"),
    )

    @Test
    fun theMusicModeIsHiddenWhileSpatialAudioIsOff() {
        val values = mapOf(
            "spatialAudio" to Setting.ToggleSetting(false),
            "spatialAudioMode" to selectSetting("Music", "Podcast", value = "Music"),
            "spatialAudioMusicMode" to musicMode,
        )

        assertTrue(
            widgetSettings(
                listOf("spatialAudioMusicMode"),
                settingsSnapshot(spatialAudioCategories, values),
            ).isEmpty(),
        )
    }

    @Test
    fun theMusicModeIsEditableWhileSpatialAudioIsOn() {
        assertEquals(
            listOf(
                WidgetSetting.Editable(
                    "spatialAudioMusicMode",
                    musicMode,
                    SettingGroup.SoundEffects,
                ),
            ),
            widgetSettings(
                listOf("spatialAudioMusicMode"),
                settingsSnapshot(
                    spatialAudioCategories,
                    spatialAudio(musicMode = true, mode = "Music"),
                ),
            ),
        )
    }

    @Test
    fun theMusicModeIsHiddenWhenTheAudioModeIsNotMusic() {
        assertTrue(
            widgetSettings(
                listOf("spatialAudioMusicMode"),
                settingsSnapshot(
                    spatialAudioCategories,
                    spatialAudio(musicMode = true, mode = "Podcast"),
                ),
            ).isEmpty(),
        )
    }

    @Test
    fun aDependentSettingIsHiddenWhileItsParentIsOff() {
        val values = mapOf(
            "easyChat" to Setting.ToggleSetting(false),
            "easyChatWaitTime" to selectSetting("1", "2", value = "1"),
        )

        assertTrue(
            widgetSettings(
                listOf("easyChatWaitTime"),
                settingsSnapshot(
                    settingCategories("miscellaneous" to listOf("easyChat", "easyChatWaitTime")),
                    values,
                ),
            ).isEmpty(),
        )
    }

    @Test
    fun availabilityWithOneEarbudKeepsTheValueAndItsReason() {
        val values = mapOf(
            "twsStatus" to Setting.InformationSetting("Disconnected", "Disconnected"),
            "ldac" to Setting.ToggleSetting(true),
        )

        assertEquals(
            listOf(
                WidgetSetting.ReadOnly(
                    "ldac",
                    Setting.ToggleSetting(true),
                    AvailabilityReason.REQUIRES_BOTH_EARBUDS,
                    SettingGroup.SoundEffects,
                ),
            ),
            widgetSettings(
                listOf("ldac"),
                settingsSnapshot(settingCategories("miscellaneous" to listOf("ldac")), values),
            ),
        )
    }

    @Test
    fun anUnavailableSettingShowsOnlyTheReason() {
        val values = mapOf(
            "twsStatus" to Setting.InformationSetting("Disconnected", "Disconnected"),
            "volumeAdjustments" to Setting.ToggleSetting(true),
        )

        assertEquals(
            listOf(
                WidgetSetting.Blocked(
                    "volumeAdjustments",
                    AvailabilityReason.REQUIRES_BOTH_EARBUDS,
                    SettingGroup.Equalizer,
                ),
            ),
            widgetSettings(
                listOf("volumeAdjustments"),
                settingsSnapshot(settingCategories("equalizer" to listOf("volumeAdjustments")), values),
            ),
        )
    }

    @Test
    fun availableSettingsKeepTheirControl() {
        val values = mapOf("wearingDetection" to Setting.ToggleSetting(true))

        assertEquals(
            listOf(WidgetSetting.Editable("wearingDetection", Setting.ToggleSetting(true), SettingGroup.Comfort)),
            widgetSettings(
                listOf("wearingDetection"),
                settingsSnapshot(settingCategories("miscellaneous" to listOf("wearingDetection")), values),
            ),
        )
    }

    @Test
    fun settingsTheDeviceDoesNotReportAreSkipped() {
        assertTrue(
            widgetSettings(
                listOf("somethingNew"),
                settingsSnapshot(
                    spatialAudioCategories,
                    spatialAudio(musicMode = true, mode = "Music"),
                ),
            ).isEmpty(),
        )
    }

    @Test
    fun rowsFollowTheSharedGroupOrderAndNotTheSelectionOrder() {
        val values = mapOf(
            "wearingDetection" to Setting.ToggleSetting(true),
            "ldac" to Setting.ToggleSetting(true),
            "ambientSoundMode" to selectSetting("Normal", "Transparency", value = "Normal"),
        )
        val rows = widgetSettings(
            listOf("wearingDetection", "ldac", "ambientSoundMode"),
            settingsSnapshot(
                settingCategories(
                    "miscellaneous" to listOf("wearingDetection", "ldac"),
                    "soundModes" to listOf("ambientSoundMode"),
                ),
                values,
            ),
        )

        assertEquals(
            listOf("ambientSoundMode", "ldac", "wearingDetection"),
            rows.map { it.settingId },
        )
        assertEquals(
            listOf(SettingGroup.SoundModes, SettingGroup.SoundEffects, SettingGroup.Comfort),
            rows.map { it.group },
        )
        assertEquals(3, rows.groups().size)
    }

    @Test
    fun theHeaderJoinsBatteriesAndShowsTheMissingEarbud() {
        // with one earbud in the case the device reports no level for the other one
        val values = mapOf(
            "twsStatus" to Setting.InformationSetting("Disconnected", "Disconnected"),
            "batteryLevelLeft" to Setting.InformationSetting("50/100", "50%"),
        )

        assertEquals(WidgetHeader(listOf("50%", "—"), oneEarbudOnly = true), widgetHeader(values))
    }

    @Test
    fun theHeaderKeepsBothBatteriesWhileBothEarbudsAreConnected() {
        val values = mapOf(
            "twsStatus" to Setting.InformationSetting("Connected", "Connected"),
            "batteryLevelLeft" to Setting.InformationSetting("50/100", "50%"),
            "batteryLevelRight" to Setting.InformationSetting("100/100", "100%"),
        )

        assertEquals(WidgetHeader(listOf("50%", "100%"), oneEarbudOnly = false), widgetHeader(values))
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
    fun aCachedSnapshotWithTheNewFieldsSurvivesSerialization() {
        val state: SettingWidgetState = SettingWidgetState.Connected(
            deviceName = "Liberty 4 Pro",
            rows = listOf(
                WidgetSetting.Editable("wearingDetection", Setting.ToggleSetting(true), SettingGroup.Comfort),
                WidgetSetting.Blocked("volumeAdjustments", AvailabilityReason.REQUIRES_BOTH_EARBUDS),
            ),
            deviceModel = "SoundcoreA3954",
            batteryValues = listOf("50%", "—"),
            oneEarbudOnly = true,
        )

        assertEquals(state, Json.decodeFromString<SettingWidgetState>(Json.encodeToString(state)))
    }

    @Test
    fun aCorruptedCachedSnapshotIsDroppedInsteadOfCrashing() {
        assertNull(decodeWidgetState(null))
        assertNull(decodeWidgetState("{"))
        assertEquals(
            SettingWidgetState.Disconnected(emptyList()),
            decodeWidgetState(
                """{"type":"com.oppzippy.openscq30.widget.SettingWidgetState.Disconnected","pairedDevices":[]}""",
            ),
        )
    }
}
