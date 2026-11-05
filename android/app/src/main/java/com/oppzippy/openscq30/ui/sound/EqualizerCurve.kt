package com.oppzippy.openscq30.ui.sound

import androidx.compose.ui.geometry.Offset

/**
 * The cubic segments through [points], smoothed so the curve follows the bands instead of stepping between them.
 *
 * The slope at each point comes from its two neighbouring slopes: a harmonic mean where they agree, and zero at a
 * turning point. A rising stretch keeps rising and the curve never reaches past the values it joins — a spline with
 * fitted slopes would overshoot, drawing a dip or a peak the device does not have, which on an equalizer reads as a
 * band set to a value it is not.
 *
 * Fewer than two points give no curve at all.
 */
internal fun equalizerCurve(points: List<Offset>): List<CubicSegment> {
    if (points.size < 2) {
        return emptyList()
    }
    val slopes = slopesAt(points)
    return (0 until points.size - 1).map { index ->
        val start = points[index]
        val end = points[index + 1]
        val across = end.x - start.x
        CubicSegment(
            start = start,
            control1 = Offset(start.x + across / 3f, start.y + slopes[index] * across / 3f),
            control2 = Offset(end.x - across / 3f, end.y - slopes[index + 1] * across / 3f),
            end = end,
        )
    }
}

/** The slope of the curve at every point, one slope per point. */
private fun slopesAt(points: List<Offset>): List<Float> {
    val between = points.zipWithNext { start, end ->
        val across = end.x - start.x
        if (across == 0f) 0f else (end.y - start.y) / across
    }
    return List(points.size) { index ->
        val before = between.getOrNull(index - 1)
        val after = between.getOrNull(index)
        when {
            // the ends keep the slope of their only neighbouring segment
            before == null -> after ?: 0f

            after == null -> before

            before * after <= 0f -> 0f

            else -> 2f * before * after / (before + after)
        }
    }
}
