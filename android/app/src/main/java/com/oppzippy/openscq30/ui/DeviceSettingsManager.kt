package com.oppzippy.openscq30.ui

import android.util.Log
import android.widget.Toast
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.features.equalizer.storage.LegacyEqualizerProfile
import com.oppzippy.openscq30.features.equalizer.storage.LegacyEqualizerProfileDao
import com.oppzippy.openscq30.features.soundcoredevice.service.DeviceConnectionManager
import com.oppzippy.openscq30.features.statusnotification.storage.FeaturedSettingSlot
import com.oppzippy.openscq30.features.statusnotification.storage.FeaturedSettingSlotDao
import com.oppzippy.openscq30.features.statusnotification.storage.QuickPresetSlot
import com.oppzippy.openscq30.features.statusnotification.storage.QuickPresetSlotDao
import com.oppzippy.openscq30.lib.bindings.OpenScq30Exception
import com.oppzippy.openscq30.lib.bindings.OpenScq30Session
import com.oppzippy.openscq30.lib.bindings.SettingCategory
import com.oppzippy.openscq30.lib.bindings.SettingIdValuePair
import com.oppzippy.openscq30.lib.bindings.SettingWriteOutcome
import com.oppzippy.openscq30.lib.bindings.WriteOutcome
import com.oppzippy.openscq30.lib.wrapper.QuickPreset
import com.oppzippy.openscq30.lib.wrapper.Setting
import com.oppzippy.openscq30.lib.wrapper.Value
import com.oppzippy.openscq30.lib.wrapper.toValue
import com.oppzippy.openscq30.ui.sound.profilesJson
import com.oppzippy.openscq30.ui.sound.toTransferProfile
import com.oppzippy.openscq30.ui.utils.ToastHandler
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.cancel
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull

/** A write that has been sent but whose outcome the device has not reported yet. */
data class PendingSettingWrite(val requestedValue: Value, val generation: Long)

class SettingWriteException(val outcome: WriteOutcome) :
    Exception(
        when (outcome) {
            is WriteOutcome.Rejected -> "the device rejected the value"
            WriteOutcome.TimedOut -> "the device did not confirm the value"
            WriteOutcome.Disconnected -> "the device disconnected"
            is WriteOutcome.UnsupportedInState -> "the setting cannot be changed in the current state"
            is WriteOutcome.Applied -> "the device applied the value"
        },
    )

class DeviceSettingsManager(
    session: OpenScq30Session,
    parent: Job,
    val deviceManager: DeviceConnectionManager,
    private val quickPresetSlotDao: QuickPresetSlotDao,
    private val featuredSettingSlotDao: FeaturedSettingSlotDao,
    legacyEqualizerProfileDao: LegacyEqualizerProfileDao,
    private val toastHandler: ToastHandler,
) : AutoCloseable {
    private val coroutineScope = CoroutineScope(Job(parent))
    private val quickPresetHandler = session.quickPresetHandler()
    private val device = deviceManager.device

    private val _quickPresetsFlow = MutableStateFlow(emptyList<QuickPreset>())
    val quickPresetsFlow = _quickPresetsFlow.asStateFlow()

    private val _pendingWrites = MutableStateFlow<Map<String, PendingSettingWrite>>(emptyMap())
    val pendingWrites = _pendingWrites.asStateFlow()

    private val _settingWriteErrors = MutableStateFlow<Map<String, WriteOutcome>>(emptyMap())
    val settingWriteErrors = _settingWriteErrors.asStateFlow()

    private var lastGeneration = 0L

    companion object {
        const val TAG = "DeviceSettingsManager"

        /** How long an applied value may take to appear in the confirmed state before the pending entry is cleared. */
        private const val CONFIRMED_STATE_TIMEOUT_MS = 2_000L
    }

    init {
        coroutineScope.launch { refreshQuickPresets() }
    }

    override fun close() {
        coroutineScope.cancel()
        quickPresetHandler.close()
    }

    fun setSettingValues(settingValues: List<Pair<String, Value>>) {
        Log.v(TAG, "Setting settings: $settingValues")
        coroutineScope.launch {
            try {
                writeSettings(settingValues)
            } catch (ex: OpenScq30Exception) {
                Log.e(TAG, "error setting values of settings", ex)
                toastHandler.add(R.string.error_changing_settings, Toast.LENGTH_SHORT)
            } catch (ex: IllegalStateException) {
                Log.w(TAG, "device was closed, not setting values", ex)
            }
        }
    }

    /**
     * Sets a single setting like [setSettingValues], but waits for the device and reports the failure to the caller
     * instead of only showing a generic toast, so that the UI can show what went wrong next to what the user entered.
     */
    suspend fun setSettingValueForResult(settingId: String, value: Value): Result<Unit> = try {
        val outcomes = writeSettings(listOf(settingId to value))
        when (val outcome = outcomes.firstOrNull { it.settingId == settingId }?.outcome) {
            is WriteOutcome.Applied -> Result.success(Unit)
            null -> Result.failure(IllegalStateException("the device reported no outcome for $settingId"))
            else -> Result.failure(SettingWriteException(outcome))
        }
    } catch (ex: OpenScq30Exception) {
        Log.e(TAG, "error setting value of $settingId", ex)
        Result.failure(ex)
    } catch (ex: IllegalStateException) {
        Log.w(TAG, "device was closed, not setting value of $settingId", ex)
        Result.failure(ex)
    }

    private suspend fun writeSettings(settingValues: List<Pair<String, Value>>): List<SettingWriteOutcome> {
        if (settingValues.isEmpty()) return emptyList()
        val generation = ++lastGeneration
        val settingIds = settingValues.map { it.first }.toSet()
        _settingWriteErrors.update { it - settingIds }
        _pendingWrites.update { pending ->
            pending + settingValues.associate { (settingId, value) ->
                settingId to PendingSettingWrite(value, generation)
            }
        }
        val outcomes = try {
            device.setSettingValues(
                settingValues.map { (settingId, value) -> SettingIdValuePair(settingId, value) },
            )
        } catch (ex: Exception) {
            clearPending(settingIds, generation)
            throw ex
        }
        applyOutcomes(generation, outcomes)
        return outcomes
    }

    private suspend fun applyOutcomes(generation: Long, outcomes: List<SettingWriteOutcome>) {
        for (settingOutcome in outcomes) {
            val settingId = settingOutcome.settingId
            val pending = _pendingWrites.value[settingId]
            // A newer write supersedes the pending entry; an older outcome must not clear or fail it.
            if (pending?.generation != generation) continue
            when (val outcome = settingOutcome.outcome) {
                is WriteOutcome.Applied -> {
                    awaitConfirmed(settingId, outcome.observed)
                    clearPending(settingId, generation)
                    _settingWriteErrors.update { it - settingId }
                }

                is WriteOutcome.Rejected -> {
                    outcome.observed?.let { awaitConfirmed(settingId, it) }
                    clearPending(settingId, generation)
                    _settingWriteErrors.update { it + (settingId to outcome) }
                }

                is WriteOutcome.UnsupportedInState -> {
                    clearPending(settingId, generation)
                    _settingWriteErrors.update { it + (settingId to outcome) }
                }

                WriteOutcome.TimedOut -> {
                    clearPending(settingId, generation)
                    _settingWriteErrors.update { it + (settingId to outcome) }
                }

                // The connection UI takes over; there is no confirmed value to show.
                WriteOutcome.Disconnected -> clearPending(settingId, generation)
            }
        }
    }

    private suspend fun awaitConfirmed(settingId: String, observed: Value) {
        fun confirmed(): Boolean = try {
            device.setting(settingId)?.toValue() == observed
        } catch (ex: IllegalStateException) {
            false
        }
        if (confirmed()) return
        withTimeoutOrNull(CONFIRMED_STATE_TIMEOUT_MS) {
            deviceManager.watchForChangeNotification.first { confirmed() }
        }
    }

    private fun clearPending(settingId: String, generation: Long) {
        _pendingWrites.update { pending ->
            if (pending[settingId]?.generation == generation) pending - settingId else pending
        }
    }

    private fun clearPending(settingIds: Set<String>, generation: Long) {
        _pendingWrites.update { pending ->
            pending.filterNot { (settingId, write) ->
                settingId in settingIds && write.generation == generation
            }
        }
    }

    val settingsByCategoryFlow: StateFlow<List<SettingCategory>?> = deviceManager.watchForChangeNotification.map {
        val settings = try {
            device.settingsByCategory()
        } catch (ex: IllegalStateException) {
            Log.w(TAG, "device was closed, can't read settings", ex)
            emptyList()
        }
        currentCoroutineContext().ensureActive()
        settings
    }.flowOn(Dispatchers.Default).stateIn(coroutineScope, SharingStarted.WhileSubscribed(), null)

    val allSettingsFlow: Flow<List<Pair<String, Setting>>> = settingsByCategoryFlow.filterNotNull().map { categories ->
        categories.flatMap { it.settings }.distinctBy { it.settingId }.map { it.settingId to it.setting }
    }

    val featuredSettingSlots = featuredSettingSlotDao.allSettingIds(device.model()).map { slots ->
        // Show a fixed number of slots
        List(2) { slots.getOrNull(it) }
    }

    fun setFeaturedSettingSlot(index: Int, settingId: String?) {
        coroutineScope.launch {
            if (settingId != null) {
                featuredSettingSlotDao.upsert(FeaturedSettingSlot(device.model(), index, settingId))
            } else {
                featuredSettingSlotDao.delete(device.model(), index)
            }
        }
    }

    val quickPresetSlots = quickPresetSlotDao.allNames(device.model()).map { slots ->
        // Show a fixed number of slots
        List(2) { slots.getOrNull(it) }
    }

    fun setQuickPresetSlot(index: Int, name: String?) {
        coroutineScope.launch {
            if (name != null) {
                quickPresetSlotDao.upsert(QuickPresetSlot(device.model(), index, name))
            } else {
                quickPresetSlotDao.delete(device.model(), index)
            }
        }
    }

    fun activateQuickPreset(name: String) {
        coroutineScope.launch {
            try {
                quickPresetHandler.activate(device, name)
            } catch (ex: OpenScq30Exception) {
                Log.e(TAG, "error activating quick preset", ex)
                toastHandler.add(R.string.error_activating_quick_preset, Toast.LENGTH_SHORT)
            }
            refreshQuickPresets()
        }
    }

    fun createQuickPreset(name: String) {
        coroutineScope.launch {
            try {
                quickPresetHandler.save(device, name)
            } catch (ex: OpenScq30Exception) {
                Log.e(TAG, "error saving quick preset", ex)
                toastHandler.add(R.string.error_saving_quick_preset, Toast.LENGTH_SHORT)
            }
            refreshQuickPresets()
        }
    }

    fun deleteQuickPreset(name: String) {
        coroutineScope.launch {
            try {
                quickPresetHandler.delete(device, name)
                // The notification builds its buttons from these slots, so they must not outlive the preset
                quickPresetSlotDao.deleteByName(device.model(), name)
            } catch (ex: OpenScq30Exception) {
                Log.e(TAG, "error deleting quick preset", ex)
                toastHandler.add(R.string.error_deleting_quick_preset, Toast.LENGTH_SHORT)
            }
            refreshQuickPresets()
        }
    }

    fun toggleQuickPresetSetting(name: String, settingId: String, enabled: Boolean) {
        coroutineScope.launch {
            try {
                quickPresetHandler.toggleField(device, name, settingId, enabled)
                // TODO either modify in place instead of re-fetching all presets, or fetch only the modified preset
                refreshQuickPresets()
            } catch (ex: OpenScq30Exception) {
                Log.e(TAG, "error changing quick preset setting", ex)
                toastHandler.add(R.string.error_changing_settings, Toast.LENGTH_SHORT)
            } catch (ex: IllegalStateException) {
                Log.w(TAG, "device was closed, not changing quick preset setting", ex)
            }
        }
    }

    private suspend fun refreshQuickPresets() {
        _quickPresetsFlow.value = try {
            quickPresetHandler.quickPresets(device).sortedBy { it.name }
        } catch (ex: OpenScq30Exception) {
            Log.w(TAG, "error fetching quick presets", ex)
            emptyList()
        } catch (ex: IllegalStateException) {
            Log.w(TAG, "device was closed, can't fetch quick presets", ex)
            emptyList()
        }
    }

    val legacyEqualizerProfilesFlow = legacyEqualizerProfileDao.all()

    /**
     * Saves old profiles as custom profiles of this device by importing them, so the device keeps its sound. Saving
     * the device's curve under each name would not work: the library keeps only the last value of each setting in a
     * batch, so putting the old values on the device, saving and restoring them would save the current curve.
     */
    fun migrateLegacyEqualizerProfiles(legacyEqualizerProfiles: List<LegacyEqualizerProfile>) {
        if (legacyEqualizerProfiles.isEmpty()) return
        coroutineScope.launch {
            val json = profilesJson(legacyEqualizerProfiles.map { it.toTransferProfile() })
            setSettingValueForResult("importCustomEqualizerProfiles", json.toValue()).onFailure {
                toastHandler.add(R.string.error_migrating_legacy_profile, Toast.LENGTH_SHORT)
            }
        }
    }
}
