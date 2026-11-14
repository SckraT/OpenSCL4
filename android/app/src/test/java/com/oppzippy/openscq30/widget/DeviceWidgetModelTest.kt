package com.oppzippy.openscq30.widget

import com.oppzippy.openscq30.lib.wrapper.Select
import com.oppzippy.openscq30.lib.wrapper.Setting
import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

private fun select(vararg options: String, value: String): Setting =
    Setting.SelectSetting(Select(options.toList(), options.toList()), value)

private fun soundModes(vararg options: String, value: String): Map<String, Setting> = mapOf(
    "ambientSoundMode" to select(*options, value = value),
)

class DeviceWidgetModelTest {
    @Test
    fun theBatteryKeepsEveryEarbudPositionWithAMissingReading() {
        val snapshot = mapOf(
            "twsStatus" to Setting.InformationSetting("Connected", "Connected"),
            "batteryLevelLeft" to Setting.InformationSetting("80/100", "80%"),
            "batteryLevelRight" to Setting.InformationSetting("90/100", "90%"),
        )

        assertEquals(
            listOf(
                WidgetBattery(WidgetBatteryCell.Left, "80%", 0.8f),
                WidgetBattery(WidgetBatteryCell.Right, "90%", 0.9f),
                WidgetBattery(WidgetBatteryCell.Case, WIDGET_UNKNOWN_VALUE, null),
            ),
            widgetBatteryCells(snapshot),
        )
    }

    @Test
    fun oneEarbudKeepsTheMissingPositionWithoutABar() {
        val snapshot = mapOf(
            "twsStatus" to Setting.InformationSetting("Disconnected", "Disconnected"),
            "batteryLevelLeft" to Setting.InformationSetting("50/100", "50%"),
        )

        assertEquals(
            listOf(
                WidgetBattery(WidgetBatteryCell.Left, "50%", 0.5f),
                WidgetBattery(WidgetBatteryCell.Right, WIDGET_UNKNOWN_VALUE, null),
                WidgetBattery(WidgetBatteryCell.Case, WIDGET_UNKNOWN_VALUE, null),
            ),
            widgetBatteryCells(snapshot),
        )
    }

    @Test
    fun asingleBatteryModelShowsItsOverallChargeInOneCell() {
        val snapshot = mapOf(
            "batteryLevel" to Setting.InformationSetting("70/100", "70%"),
        )

        assertEquals(
            listOf(WidgetBattery(WidgetBatteryCell.Whole, "70%", 0.7f)),
            widgetBatteryCells(snapshot),
        )
    }

    @Test
    fun anEmptySnapshotHasNoInventedReadings() {
        assertEquals(
            listOf(WidgetBattery(WidgetBatteryCell.Whole, WIDGET_UNKNOWN_VALUE, null)),
            widgetBatteryCells(emptyMap()),
        )
    }

    @Test
    fun theSwitcherGoesToNoiseCancelingFromAnyOtherMode() {
        for (current in listOf(WIDGET_MODE_TRANSPARENCY, WIDGET_MODE_NORMAL)) {
            val modes = widgetSoundModes(
                soundModes(WIDGET_MODE_NOISE_CANCELING, WIDGET_MODE_TRANSPARENCY, WIDGET_MODE_NORMAL, value = current),
            )
            assertEquals(WIDGET_MODE_NOISE_CANCELING, modes.toggleTarget)
        }
    }

    @Test
    fun theSwitcherGoesToTransparencyFromNoiseCanceling() {
        val modes = widgetSoundModes(
            soundModes(
                WIDGET_MODE_NOISE_CANCELING,
                WIDGET_MODE_TRANSPARENCY,
                WIDGET_MODE_NORMAL,
                value = WIDGET_MODE_NOISE_CANCELING,
            ),
        )

        assertEquals(WIDGET_MODE_TRANSPARENCY, modes.toggleTarget)
    }

    @Test
    fun withoutAConflictedModeSelectionTheSwitcherCannotChangeAnything() {
        val modes = widgetSoundModes(emptyMap())

        assertTrue(modes.modes.isEmpty())
        assertFalse(modes.canSwitch)
        assertNull(modes.toggleTarget)
    }

    @Test
    fun theRowDrawsOnlyTheThreeActionModesInThePresentationOrder() {
        val modes = widgetSoundModes(
            soundModes(
                WIDGET_MODE_NOISE_CANCELING,
                WIDGET_MODE_TRANSPARENCY,
                WIDGET_MODE_NORMAL,
                WIDGET_MODE_AIRPLANE,
                value = WIDGET_MODE_NORMAL,
            ),
        )

        assertEquals(
            listOf(WIDGET_MODE_NOISE_CANCELING, WIDGET_MODE_TRANSPARENCY, WIDGET_MODE_NORMAL),
            modes.actionModes.map { it.id },
        )
        assertTrue(modes.isSelected(WIDGET_MODE_NORMAL))
    }

    @Test
    fun airplaneModeSelectsNoneOfTheThreeRowButtons() {
        val modes = widgetSoundModes(
            soundModes(
                WIDGET_MODE_NOISE_CANCELING,
                WIDGET_MODE_TRANSPARENCY,
                WIDGET_MODE_NORMAL,
                WIDGET_MODE_AIRPLANE,
                value = WIDGET_MODE_AIRPLANE,
            ),
        )

        assertTrue(modes.actionModes.none { modes.isSelected(it.id) })
    }

    @Test
    fun thePresetSlotsKeepTheirOrderAndMarkTheLastAppliedPreset() {
        val state = connectedWidgetState(
            deviceName = "Liberty 4 Pro",
            deviceModel = "SoundcoreA3954",
            snapshot = mapOf("wearingDetection" to Setting.ToggleSetting(true)),
            presetSlots = listOf("Дорога", "Разговор"),
            availablePresetNames = setOf("Дорога", "Разговор", "Другое"),
            lastAppliedPreset = "Разговор",
        )

        assertEquals(
            listOf(WidgetPreset("Дорога"), WidgetPreset("Разговор", lastApplied = true)),
            state.presets,
        )
    }

    @Test
    fun aDeletedPresetLeavesItsSlotAndAnUnknownListKeepsTheSlots() {
        val deleted = connectedWidgetState(
            deviceName = "Liberty 4 Pro",
            deviceModel = "SoundcoreA3954",
            snapshot = emptyMap(),
            presetSlots = listOf("Дорога", "Удалён"),
            availablePresetNames = setOf("Дорога"),
            lastAppliedPreset = null,
        )
        assertEquals(listOf("Дорога"), deleted.presets.map { it.name })

        val unknown = connectedWidgetState(
            deviceName = "Liberty 4 Pro",
            deviceModel = "SoundcoreA3954",
            snapshot = emptyMap(),
            presetSlots = listOf("Дорога", "Удалён"),
            availablePresetNames = null,
            lastAppliedPreset = null,
        )
        assertEquals(listOf("Дорога", "Удалён"), unknown.presets.map { it.name })
    }

    @Test
    fun theWidgetStateSurvivesSerialization() {
        val state = connectedWidgetState(
            deviceName = "Liberty 4 Pro",
            deviceModel = "SoundcoreA3954",
            snapshot = soundModes(
                WIDGET_MODE_NOISE_CANCELING,
                WIDGET_MODE_TRANSPARENCY,
                value = WIDGET_MODE_NOISE_CANCELING,
            ),
            presetSlots = listOf("Дорога"),
            availablePresetNames = setOf("Дорога"),
            lastAppliedPreset = "Дорога",
        )

        val json = Json.encodeToString<DeviceWidgetState>(state)
        assertEquals(state, Json.decodeFromString<DeviceWidgetState>(json))
        assertNull(decodeDeviceWidgetState("{"))
    }

    @Test
    fun presetSlotsKeepThePickOrderAndTheLimit() {
        assertEquals(listOf("A"), togglePresetSelection(emptyList(), "A", 2))
        assertEquals(listOf("A", "B"), togglePresetSelection(listOf("A"), "B", 2))
        assertEquals(listOf("A"), togglePresetSelection(listOf("A", "B"), "B", 2))
        assertEquals(listOf("A", "B"), togglePresetSelection(listOf("A", "B"), "C", 2))
    }
}
