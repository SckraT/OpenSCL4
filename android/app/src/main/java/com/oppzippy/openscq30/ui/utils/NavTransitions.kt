package com.oppzippy.openscq30.ui.utils

import androidx.compose.animation.AnimatedContentTransitionScope
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.ui.unit.IntOffset
import androidx.navigation.NavBackStackEntry

private val slideSpec = tween<IntOffset>(250, easing = FastOutSlowInEasing)
private val fadeSpec = tween<Float>(200, easing = FastOutSlowInEasing)

/**
 * Page changes in every NavHost: a short slide with a fade, forwards to the left and back to the right. Predictive back
 * plays the back transition along with the gesture, so it finishes quickly once the finger lets go.
 */
object NavTransitions {
    val enter: AnimatedContentTransitionScope<NavBackStackEntry>.() -> EnterTransition = {
        slideInHorizontally(slideSpec) { it / 4 } + fadeIn(fadeSpec)
    }
    val exit: AnimatedContentTransitionScope<NavBackStackEntry>.() -> ExitTransition = {
        slideOutHorizontally(slideSpec) { -it / 4 } + fadeOut(fadeSpec)
    }
    val popEnter: AnimatedContentTransitionScope<NavBackStackEntry>.() -> EnterTransition = {
        slideInHorizontally(slideSpec) { -it / 4 } + fadeIn(fadeSpec)
    }
    val popExit: AnimatedContentTransitionScope<NavBackStackEntry>.() -> ExitTransition = {
        slideOutHorizontally(slideSpec) { it / 4 } + fadeOut(fadeSpec)
    }

    // A back swipe has transitions of its own (by default the old page shrinks slowly); use the same back motion.
    val predictivePopEnter: AnimatedContentTransitionScope<NavBackStackEntry>.(Int) -> EnterTransition = {
        popEnter(this)
    }
    val predictivePopExit: AnimatedContentTransitionScope<NavBackStackEntry>.(Int) -> ExitTransition = {
        popExit(this)
    }
}
