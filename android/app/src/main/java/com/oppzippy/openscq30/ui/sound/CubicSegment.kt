package com.oppzippy.openscq30.ui.sound

import androidx.compose.ui.geometry.Offset

/** One piece of the curve drawn through the equalizer's bands: where it starts, bends and ends. */
internal data class CubicSegment(val start: Offset, val control1: Offset, val control2: Offset, val end: Offset)
