package com.oppzippy.openscq30.features.battery

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.ViewModel
import dagger.hilt.android.lifecycle.HiltViewModel
import javax.inject.Inject
import kotlinx.coroutines.delay

@HiltViewModel
class SystemBatteryViewModel @Inject constructor(private val provider: SystemBatteryProvider) : ViewModel() {
    fun percent(macAddress: String): Int? = provider.percent(macAddress)
}

/** How often the system value is read: Android updates it slowly, and asking more often costs nothing but adds noise. */
private const val SYSTEM_BATTERY_REFRESH_MS = 60_000L

/**
 * The system percentage while [enabled], read once on connect and then every minute, or null when it is off or
 * Android has no reading.
 */
@Composable
fun rememberSystemBatteryPercent(
    macAddress: String?,
    enabled: Boolean,
    viewModel: SystemBatteryViewModel = hiltViewModel(),
): Int? {
    var percent by remember { mutableStateOf<Int?>(null) }
    LaunchedEffect(macAddress, enabled) {
        percent = null
        if (!enabled || macAddress == null) return@LaunchedEffect
        while (true) {
            percent = viewModel.percent(macAddress)
            delay(SYSTEM_BATTERY_REFRESH_MS)
        }
    }
    return percent
}
