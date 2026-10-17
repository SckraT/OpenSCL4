package com.oppzippy.openscq30.ui.sound

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class OverviewLayoutTest {
    @Test
    fun nothingStoredIsTheStandardSet() {
        assertEquals(OverviewLayout.of(OverviewPreset.Standard), OverviewLayout.parse(null))
        assertEquals(OverviewPreset.Standard, OverviewLayout.parse("").preset)
        assertEquals(false, OverviewLayout.Default.blocks.toMap()[OverviewBlock.QuickPresets])
    }

    @Test
    fun aLayoutSurvivesSaving() {
        val layout = OverviewLayout.Default.moved(0, 3).withVisibility(OverviewBlock.Case, false)
        assertEquals(layout, OverviewLayout.parse(layout.serialize()))
    }

    @Test
    fun unknownBlocksAreSkippedAndNewOnesKeepTheirDefault() {
        val layout = OverviewLayout.parse("equalizer,!soundMode,removedBlock")
        assertEquals(listOf(OverviewBlock.Equalizer, OverviewBlock.SoundMode), layout.blocks.take(2).map { it.first })
        assertEquals(false, layout.blocks.toMap()[OverviewBlock.SoundMode])
        assertEquals(OverviewBlock.entries.toSet(), layout.blocks.map { it.first }.toSet())
        assertEquals(true, layout.blocks.toMap()[OverviewBlock.Controls])
    }

    @Test
    fun presetsShowTheirBlocks() {
        assertEquals(
            listOf(OverviewBlock.SoundMode, OverviewBlock.Equalizer),
            OverviewLayout.of(OverviewPreset.Minimal).visibleBlocks,
        )
        assertEquals(OverviewBlock.entries, OverviewLayout.of(OverviewPreset.Full).visibleBlocks)
        assertEquals(OverviewPreset.Full, OverviewLayout.of(OverviewPreset.Full).preset)
    }

    @Test
    fun aChangedLayoutIsNoLongerAPreset() {
        assertNull(OverviewLayout.Default.withVisibility(OverviewBlock.Spatial, false).preset)
        assertNull(OverviewLayout.Default.moved(0, 1).preset)
    }

    @Test
    fun movingOutsideTheListChangesNothing() {
        assertEquals(OverviewLayout.Default, OverviewLayout.Default.moved(0, 99))
    }
}
