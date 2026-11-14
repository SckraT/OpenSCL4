package com.oppzippy.openscq30.ui.theme

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ThemePreferenceTest {
    @Test
    fun followingTheSystemTakesTheSystemTheme() {
        val theme: ThemeType? = null

        assertFalse(theme.isDark(systemInDark = false))
        assertTrue(theme.isDark(systemInDark = true))
    }

    @Test
    fun theLightThemeIsNeverDark() {
        assertFalse(ThemeType.Light.isDark(systemInDark = false))
        assertFalse(ThemeType.Light.isDark(systemInDark = true))
    }

    @Test
    fun theDarkThemeIsAlwaysDark() {
        assertTrue(ThemeType.Dark.isDark(systemInDark = false))
        assertTrue(ThemeType.Dark.isDark(systemInDark = true))
    }
}
