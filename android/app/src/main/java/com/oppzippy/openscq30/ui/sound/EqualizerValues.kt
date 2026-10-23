package com.oppzippy.openscq30.ui.sound

import com.oppzippy.openscq30.lib.bindings.clientFormatEqualizerValue
import com.oppzippy.openscq30.lib.bindings.clientParseEqualizerValue
import com.oppzippy.openscq30.lib.wrapper.Equalizer

internal fun formatEqualizerValue(value: Short, fractionDigits: Short): String =
    clientFormatEqualizerValue(value, fractionDigits)

internal fun parseEqualizerValue(text: String, equalizer: Equalizer): Short? =
    clientParseEqualizerValue(text, equalizer)

/** Shows a requested write while it is pending, but only if it still fits the confirmed band count. */
internal fun displayedEqualizerValues(confirmed: List<Short>, pending: List<Short>?): List<Short> =
    pending?.takeIf { it.size == confirmed.size } ?: confirmed
