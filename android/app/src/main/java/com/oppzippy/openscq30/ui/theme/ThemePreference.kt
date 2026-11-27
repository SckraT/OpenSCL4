package com.oppzippy.openscq30.ui.theme

import android.content.Context
import android.content.res.Configuration

/** Whether a theme preference draws the dark theme; following the system takes [systemInDark]. */
internal fun ThemeType?.isDark(systemInDark: Boolean): Boolean = when (this) {
    null -> systemInDark
    ThemeType.Light -> false
    ThemeType.Dark -> true
}

/** Whether the system is in its dark theme, read without a Compose composition so a widget can use it. */
internal fun Context.systemInDarkTheme(): Boolean =
    (resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES
