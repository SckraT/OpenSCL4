package com.oppzippy.openscq30.ui.sound

import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.ui.theme.darkColorScheme
import com.oppzippy.openscq30.ui.theme.lightColorScheme
import org.junit.Assert.assertEquals
import org.junit.Test

class SoundEarbudsArtTest {
    @Test
    fun lightThemeShowsTheSilverEarbuds() {
        assertEquals(R.drawable.sound_liberty_4_pro_silver, earbudsArtImage(lightColorScheme))
    }

    @Test
    fun darkThemeShowsTheBlackEarbuds() {
        assertEquals(R.drawable.sound_liberty_4_pro_black, earbudsArtImage(darkColorScheme))
    }
}
