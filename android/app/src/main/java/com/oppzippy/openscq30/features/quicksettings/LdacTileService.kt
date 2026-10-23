package com.oppzippy.openscq30.features.quicksettings

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.os.Build
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.features.soundcoredevice.service.DeviceService
import com.oppzippy.openscq30.features.soundcoredevice.service.LdacTileState
import com.oppzippy.openscq30.lib.wrapper.blockedReason
import com.oppzippy.openscq30.lib.wrapper.isAvailable
import com.oppzippy.openscq30.lib.wrapper.toValue
import com.oppzippy.openscq30.ui.sound.availabilityReasonText
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch

/**
 * A Quick Settings tile that turns LDAC on and off on the connected device without opening the app. It is unavailable
 * while no device is connected or the device has no LDAC. With one earbud it shows the current value and the reason it
 * cannot be switched.
 */
class LdacTileService : TileService() {
    companion object {
        /** Asks the system to refresh the tile, for example when the device's LDAC setting changes. */
        fun requestUpdate(context: Context) {
            requestListeningState(context, ComponentName(context, LdacTileService::class.java))
        }
    }

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private var listening: Job? = null

    override fun onStartListening() {
        super.onStartListening()
        listening?.cancel()
        listening = scope.launch { DeviceService.ldacState.collect { update(it) } }
    }

    override fun onStopListening() {
        listening?.cancel()
        listening = null
        super.onStopListening()
    }

    override fun onDestroy() {
        // calls onStopListening when the tile is still listening
        super.onDestroy()
        scope.cancel()
    }

    override fun onClick() {
        super.onClick()
        val state = DeviceService.ldacState.value ?: return
        if (!state.availability.isAvailable()) return
        sendBroadcast(
            Intent(DeviceService.ACTION_SET_SETTING_VALUE)
                .setPackage(packageName)
                .putExtra(DeviceService.INTENT_EXTRA_SETTING_ID, "ldac")
                .putExtra(DeviceService.INTENT_EXTRA_SETTING_VALUE, (!state.enabled).toValue()),
        )
    }

    private fun update(state: LdacTileState?) {
        val tile = qsTile ?: return
        tile.label = getString(R.string.sound_ldac_tile)
        tile.state = when (state?.enabled) {
            null -> Tile.STATE_UNAVAILABLE
            true -> Tile.STATE_ACTIVE
            false -> Tile.STATE_INACTIVE
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            val reason = state?.availability?.blockedReason()
            tile.subtitle = when {
                state == null -> getString(R.string.sound_ldac_tile_unavailable)
                reason != null -> getString(availabilityReasonText(reason))
                else -> null
            }
        }
        tile.updateTile()
    }
}
