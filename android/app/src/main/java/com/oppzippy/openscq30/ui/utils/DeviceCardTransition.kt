@file:OptIn(ExperimentalSharedTransitionApi::class)

package com.oppzippy.openscq30.ui.utils

import androidx.compose.animation.AnimatedVisibilityScope
import androidx.compose.animation.ExperimentalSharedTransitionApi
import androidx.compose.animation.SharedTransitionScope
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.runtime.Composable
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.IntOffset

/**
 * The device card on My devices becomes the card on top of the device overview and back. The root of the app provides
 * the transition scopes; without them (previews, tests) these modifiers do nothing.
 */
val LocalSharedTransitionScope = compositionLocalOf<SharedTransitionScope?> { null }
val LocalRootAnimatedVisibilityScope = compositionLocalOf<AnimatedVisibilityScope?> { null }

private fun cardKey(part: String, macAddress: String) = "device-$part-${macAddress.uppercase()}"

/** The card itself: its bounds move and resize between the two screens while its content cross-fades. */
@Composable
fun Modifier.sharedDeviceCard(macAddress: String?): Modifier {
    val shared = LocalSharedTransitionScope.current ?: return this
    val visibility = LocalRootAnimatedVisibilityScope.current ?: return this
    if (macAddress == null) return this
    return with(shared) {
        this@sharedDeviceCard.sharedBounds(
            rememberSharedContentState(cardKey("card", macAddress)),
            animatedVisibilityScope = visibility,
            resizeMode = SharedTransitionScope.ResizeMode.RemeasureToBounds,
        )
    }
}

/** A part of the card that is the same on both screens, such as the illustration or the name, moves as one piece. */
@Composable
fun Modifier.sharedDevicePart(macAddress: String?, part: String): Modifier {
    val shared = LocalSharedTransitionScope.current ?: return this
    val visibility = LocalRootAnimatedVisibilityScope.current ?: return this
    if (macAddress == null) return this
    return with(shared) {
        this@sharedDevicePart.sharedElement(
            rememberSharedContentState(cardKey(part, macAddress)),
            animatedVisibilityScope = visibility,
        )
    }
}

private val slideSpec = tween<IntOffset>(300, easing = FastOutSlowInEasing)
private val fadeSpec = tween<Float>(250, easing = FastOutSlowInEasing)

/** What surrounds the card comes in from below and leaves downwards while the card changes. */
@Composable
fun Modifier.slidesWithDeviceCard(): Modifier {
    val visibility = LocalRootAnimatedVisibilityScope.current ?: return this
    return with(visibility) {
        this@slidesWithDeviceCard.animateEnterExit(
            enter = slideInVertically(slideSpec) { it / 5 } + fadeIn(fadeSpec),
            exit = slideOutVertically(slideSpec) { it / 5 } + fadeOut(fadeSpec),
        )
    }
}
