package com.oppzippy.openscq30.features.battery

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.LifecycleResumeEffect
import dagger.hilt.android.lifecycle.HiltViewModel
import javax.inject.Inject
import kotlinx.coroutines.delay

@HiltViewModel
class SystemBatteryViewModel @Inject constructor(private val provider: SystemBatteryProvider) : ViewModel() {
    fun percent(macAddress: String): Int? = provider.percent(macAddress)
}

/** How often the system value is read while the app stays on screen: Android updates it slowly. */
private const val SYSTEM_BATTERY_REFRESH_MS = 60_000L

/**
 * The system percentage while [enabled], read on connect, on every return to the app and every minute after that,
 * or null when the mode is off or Android has no reading.
 */
@Composable
fun rememberSystemBatteryPercent(
    macAddress: String?,
    enabled: Boolean,
    viewModel: SystemBatteryViewModel = hiltViewModel(),
): Int? {
    var percent by remember { mutableStateOf<Int?>(null) }
    // Android's number changes while the app is in the background, so a return to the screen re-reads it at once
    var resumes by remember { mutableIntStateOf(0) }
    LifecycleResumeEffect(macAddress, enabled) {
        resumes++
        onPauseOrDispose { }
    }
    LaunchedEffect(macAddress, enabled, resumes) {
        percent = null
        if (!enabled || macAddress == null) return@LaunchedEffect
        while (true) {
            // a missing reading is not cached: the next attempt is a minute away
            percent = viewModel.percent(macAddress)
            delay(SYSTEM_BATTERY_REFRESH_MS)
        }
    }
    return percent
}
