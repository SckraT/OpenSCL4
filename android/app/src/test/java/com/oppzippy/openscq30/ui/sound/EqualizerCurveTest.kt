package com.oppzippy.openscq30.ui.sound

import androidx.compose.ui.geometry.Offset
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class EqualizerCurveTest {
    private companion object {
        /** Coordinates are computed in floats, so a third of the way is rarely exactly on the pixel. */
        const val TOLERANCE = 0.01f
    }

    @Test
    fun twoPointsGiveTheStraightLineBetweenThem() {
        val segments = equalizerCurve(listOf(Offset(0f, 0f), Offset(30f, 60f)))

        assertEquals(1, segments.size)
        // a third of the way along the straight line, so the curve is the line itself
        assertEquals(Offset(10f, 20f), segments[0].control1)
        assertEquals(Offset(20f, 40f), segments[0].control2)
    }

    @Test
    fun aTurningPointIsFlatSoTheCurveDoesNotBulgePastIt() {
        val segments = equalizerCurve(listOf(Offset(0f, 100f), Offset(10f, 20f), Offset(20f, 100f)))

        // the slope through the peak is zero, so both controls beside it keep its height
        assertEquals(20f, segments[0].control2.y, TOLERANCE)
        assertEquals(20f, segments[1].control1.y, TOLERANCE)
    }

    @Test
    fun theCurveNeverReachesPastTheValuesItJoins() {
        // a rise onto a plateau: a spline with fitted slopes bulges past the plateau here
        val segments = equalizerCurve(
            listOf(Offset(0f, 100f), Offset(10f, 50f), Offset(20f, 50f), Offset(30f, 50f)),
        )

        segments.forEach { segment ->
            val lowest = minOf(segment.start.y, segment.end.y)
            val highest = maxOf(segment.start.y, segment.end.y)
            drawnPoints(segment).forEach { point ->
                assertTrue(
                    "$point leaves the $lowest..$highest its segment joins",
                    point.y >= lowest - TOLERANCE && point.y <= highest + TOLERANCE,
                )
            }
        }
    }

    @Test
    fun nothingToDrawHasNoCurve() {
        assertTrue(equalizerCurve(emptyList()).isEmpty())
        assertTrue(equalizerCurve(listOf(Offset(5f, 5f))).isEmpty())
    }

    /** Points along the segment, so the test checks the curve it draws and not only its controls. */
    private fun drawnPoints(segment: CubicSegment, steps: Int = 24): List<Offset> = (0..steps).map { step ->
        val t = step / steps.toFloat()
        val u = 1f - t
        Offset(
            u * u * u * segment.start.x + 3f * u * u * t * segment.control1.x +
                3f * u * t * t * segment.control2.x + t * t * t * segment.end.x,
            u * u * u * segment.start.y + 3f * u * u * t * segment.control1.y +
                3f * u * t * t * segment.control2.y + t * t * t * segment.end.y,
        )
    }
}
