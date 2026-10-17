package com.oppzippy.openscq30.ui.sound

import com.oppzippy.openscq30.features.equalizer.storage.LegacyEqualizerProfile
import com.oppzippy.openscq30.lib.wrapper.Equalizer
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class EqualizerProfileTransferTest {
    private val bands = listOf(100, 200, 400, 800, 1600, 3200, 6400, 12800).map { it.toUShort() }
    private val equalizer = Equalizer(bands, fractionDigits = 1, min = -120, max = 134)
    private fun profile(name: String, vararg values: Double) = TransferProfile(name, values.toList())
    private val flat = DoubleArray(8)

    @Test
    fun readsTheLibraryExportFormat() {
        val json = """[{"name": "test profile", "volumeAdjustments": [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7]}]"""
        assertEquals(
            listOf(profile("test profile", 0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7)),
            parseProfiles(json),
        )
    }

    @Test
    fun writtenProfilesReadBackTheSame() {
        val profiles = listOf(profile("a", *flat), profile("b", -12.0, 13.4, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0))
        assertEquals(profiles, parseProfiles(profilesJson(profiles)))
    }

    @Test
    fun textThatIsNotAProfilesFileIsRejected() {
        assertNull(parseProfiles("hello"))
        assertNull(parseProfiles("""{"name": "not a list"}"""))
        assertNull(parseProfiles("""[{"name": "no values"}]"""))
    }

    @Test
    fun previewSaysWhatImportingWouldDo() {
        val preview = previewImport(
            listOf(
                profile("new", *flat),
                profile("saved", *flat),
                profile("short", 1.0, 2.0),
                profile("loud", 20.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            ),
            existingNames = listOf("saved"),
            equalizer = equalizer,
        )
        assertEquals(
            listOf(ImportStatus.New, ImportStatus.Replaces, ImportStatus.WrongBandCount, ImportStatus.OutOfRange),
            preview.map { it.status },
        )
    }

    @Test
    fun theRangeIsCheckedAtTheDevicePrecisionLikeTheLibrary() {
        fun status(value: Double) = previewImport(
            listOf(profile("p", value, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)),
            emptyList(),
            equalizer,
        ).single().status
        assertEquals(ImportStatus.New, status(13.4))
        assertEquals(ImportStatus.New, status(-12.04))
        // -12.05 rounds away from zero to -121, past the minimum of -120
        assertEquals(ImportStatus.OutOfRange, status(-12.05))
        assertEquals(ImportStatus.OutOfRange, status(Double.NaN))
    }

    @Test
    fun withoutTheEqualizerOnlyNamesAreCompared() {
        val preview = previewImport(listOf(profile("short", 1.0), profile("saved", 1.0)), listOf("saved"), null)
        assertEquals(listOf(ImportStatus.New, ImportStatus.Replaces), preview.map { it.status })
    }

    @Test
    fun valuesAreScaledToTheDevice() {
        assertEquals(
            listOf<Short>(-120, 134, 5, 0),
            profile("p", -12.0, 13.4, 0.5, 0.0).deviceValues(fractionDigits = 1),
        )
    }

    @Test
    fun oldProfilesWithASavedProfileOfTheSameNameWereMovedAlready() {
        val old = LegacyEqualizerProfile("Old", 10, -20, 30, 40, 50, 60, 70, 80)
        val moved = LegacyEqualizerProfile("Moved", 0, 0, 0, 0, 0, 0, 0, 0)
        assertEquals(listOf(old), unmigratedLegacyProfiles(listOf(old, moved), listOf("Moved")))
        assertEquals(profile("Old", 1.0, -2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0), old.toTransferProfile())
    }
}
