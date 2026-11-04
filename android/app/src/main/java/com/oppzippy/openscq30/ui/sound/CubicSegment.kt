package com.oppzippy.openscq30.ui.sound

import androidx.compose.ui.geometry.Offset

/**
 * One piece of the curve drawn through the equalizer's bands: where it starts, bends and ends.
 *
 * It has its own file because ktlint's `standard:filename` rule wants a file named after the type it declares.
 */
internal data class CubicSegment(val start: Offset, val control1: Offset, val control2: Offset, val end: Offset)
