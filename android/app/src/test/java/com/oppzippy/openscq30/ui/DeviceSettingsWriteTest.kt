package com.oppzippy.openscq30.ui

import android.util.Log
import com.oppzippy.openscq30.features.soundcoredevice.service.DeviceConnectionManager
import com.oppzippy.openscq30.lib.bindings.AvailabilityReason
import com.oppzippy.openscq30.lib.bindings.OpenScq30Device
import com.oppzippy.openscq30.lib.bindings.OpenScq30Session
import com.oppzippy.openscq30.lib.bindings.QuickPresetsHandler
import com.oppzippy.openscq30.lib.bindings.SettingWriteOutcome
import com.oppzippy.openscq30.lib.bindings.WriteOutcome
import com.oppzippy.openscq30.lib.wrapper.Setting
import com.oppzippy.openscq30.lib.wrapper.Value
import com.oppzippy.openscq30.lib.wrapper.toValue
import com.oppzippy.openscq30.test.MainDispatcherRule
import io.mockk.coEvery
import io.mockk.every
import io.mockk.mockk
import io.mockk.mockkStatic
import io.mockk.unmockkStatic
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class DeviceSettingsWriteTest {
    @get:Rule
    val mainDispatcherRule = MainDispatcherRule()

    private val managers = mutableListOf<DeviceSettingsManager>()

    @Before
    fun setUp() {
        mockkStatic(Log::class)
        every { Log.v(any(), any()) } returns 0
        every { Log.w(any(), any<String>()) } returns 0
        every { Log.w(any(), any<String>(), any()) } returns 0
        every { Log.e(any(), any<String>()) } returns 0
        every { Log.e(any(), any<String>(), any()) } returns 0
    }

    @After
    fun tearDown() {
        managers.forEach { it.close() }
        unmockkStatic(Log::class)
    }

    private fun manager(
        device: OpenScq30Device,
        watchForChanges: MutableStateFlow<Int> = MutableStateFlow(0),
    ): DeviceSettingsManager {
        val session = mockk<OpenScq30Session>(relaxed = true)
        val presetHandler = mockk<QuickPresetsHandler>(relaxed = true)
        every { session.quickPresetHandler() } returns presetHandler
        coEvery { presetHandler.quickPresets(any()) } returns emptyList()
        val deviceManager = mockk<DeviceConnectionManager>()
        every { deviceManager.device } returns device
        every { deviceManager.watchForChangeNotification } returns watchForChanges
        return DeviceSettingsManager(
            session = session,
            parent = Job(),
            deviceManager = deviceManager,
            quickPresetSlotDao = mockk(relaxed = true),
            featuredSettingSlotDao = mockk(relaxed = true),
            legacyEqualizerProfileDao = mockk(relaxed = true),
            toastHandler = mockk(relaxed = true),
        ).also { managers += it }
    }

    private fun device(): OpenScq30Device = mockk(relaxed = true) {
        every { setting("wearingDetection") } returns Setting.ToggleSetting(false)
    }

    @Test
    fun appliedSettingClearsPendingOnlyAfterTheConfirmedStateIsVisible() = runTest {
        val device = device()
        val watchForChanges = MutableStateFlow(0)
        var confirmed = false
        every { device.setting("wearingDetection") } answers { Setting.ToggleSetting(confirmed) }
        coEvery { device.setSettingValues(any()) } returns listOf(
            SettingWriteOutcome("wearingDetection", WriteOutcome.Applied(Value.BoolValue(true))),
        )
        val manager = manager(device, watchForChanges)
        var result: Result<Unit>? = null

        val write = launch { result = manager.setSettingValueForResult("wearingDetection", Value.BoolValue(true)) }
        runCurrent()

        assertEquals(Value.BoolValue(true), manager.pendingWrites.value["wearingDetection"]?.requestedValue)
        assertFalse(write.isCompleted)

        confirmed = true
        watchForChanges.value += 1
        runCurrent()

        assertTrue(write.isCompleted)
        assertTrue(result!!.isSuccess)
        assertNull(manager.pendingWrites.value["wearingDetection"])
        assertNull(manager.settingWriteErrors.value["wearingDetection"])
    }

    @Test
    fun rejectedSettingClearsPendingAndReportsTheOutcome() = runTest {
        val device = device()
        coEvery { device.setSettingValues(any()) } returns listOf(
            SettingWriteOutcome("wearingDetection", WriteOutcome.Rejected(Value.BoolValue(false))),
        )
        val manager = manager(device)

        val result = manager.setSettingValueForResult("wearingDetection", Value.BoolValue(true))

        assertTrue(result.isFailure)
        assertNull(manager.pendingWrites.value["wearingDetection"])
        assertEquals(Value.BoolValue(false), device.setting("wearingDetection")?.toValue())
        assertEquals(
            WriteOutcome.Rejected(Value.BoolValue(false)),
            manager.settingWriteErrors.value["wearingDetection"],
        )
    }

    @Test
    fun timedOutSettingClearsPendingAndReportsTheOutcome() = runTest {
        val device = device()
        coEvery { device.setSettingValues(any()) } returns listOf(
            SettingWriteOutcome("wearingDetection", WriteOutcome.TimedOut),
        )
        val manager = manager(device)

        val result = manager.setSettingValueForResult("wearingDetection", Value.BoolValue(true))

        assertTrue(result.isFailure)
        assertNull(manager.pendingWrites.value["wearingDetection"])
        assertEquals(WriteOutcome.TimedOut, manager.settingWriteErrors.value["wearingDetection"])
    }

    @Test
    fun unsupportedSettingReportsTheReason() = runTest {
        val device = device()
        coEvery { device.setSettingValues(any()) } returns listOf(
            SettingWriteOutcome(
                "wearingDetection",
                WriteOutcome.UnsupportedInState(AvailabilityReason.REQUIRES_BOTH_EARBUDS),
            ),
        )
        val manager = manager(device)

        val result = manager.setSettingValueForResult("wearingDetection", Value.BoolValue(true))

        assertTrue(result.isFailure)
        assertNull(manager.pendingWrites.value["wearingDetection"])
        assertEquals(
            WriteOutcome.UnsupportedInState(AvailabilityReason.REQUIRES_BOTH_EARBUDS),
            manager.settingWriteErrors.value["wearingDetection"],
        )
    }

    @Test
    fun staleOutcomeDoesNotClearANewerPendingWrite() = runTest {
        val device = device()
        val first = CompletableDeferred<List<SettingWriteOutcome>>()
        val second = CompletableDeferred<List<SettingWriteOutcome>>()
        var calls = 0
        coEvery { device.setSettingValues(any()) } coAnswers {
            if (calls++ == 0) first.await() else second.await()
        }
        every { device.setting("wearingDetection") } answers { Setting.ToggleSetting(false) }
        val manager = manager(device)

        val firstWrite = launch {
            manager.setSettingValueForResult("wearingDetection", Value.BoolValue(true))
        }
        runCurrent()
        val secondWrite = launch {
            manager.setSettingValueForResult("wearingDetection", Value.BoolValue(false))
        }
        runCurrent()

        first.complete(
            listOf(SettingWriteOutcome("wearingDetection", WriteOutcome.Rejected(Value.BoolValue(false)))),
        )
        runCurrent()

        assertTrue(firstWrite.isCompleted)
        assertFalse(secondWrite.isCompleted)
        assertNotNull(manager.pendingWrites.value["wearingDetection"])
        assertNull(manager.settingWriteErrors.value["wearingDetection"])

        second.complete(
            listOf(SettingWriteOutcome("wearingDetection", WriteOutcome.Applied(Value.BoolValue(false)))),
        )
        runCurrent()

        assertTrue(secondWrite.isCompleted)
        assertNull(manager.pendingWrites.value["wearingDetection"])
        assertNull(manager.settingWriteErrors.value["wearingDetection"])
    }

    @Test
    fun fireAndForgetWriteKeepsTheRequestedValuePendingUntilTheOutcome() = runBlocking {
        val device = device()
        val outcomes = CompletableDeferred<List<SettingWriteOutcome>>()
        coEvery { device.setSettingValues(any()) } coAnswers { outcomes.await() }
        val manager = manager(device)

        manager.setSettingValues(listOf("wearingDetection" to Value.BoolValue(true)))
        withTimeout(5_000) {
            manager.pendingWrites.first { it["wearingDetection"]?.requestedValue == Value.BoolValue(true) }
        }

        outcomes.complete(
            listOf(SettingWriteOutcome("wearingDetection", WriteOutcome.Rejected(Value.BoolValue(false)))),
        )
        withTimeout(5_000) { manager.pendingWrites.first { it["wearingDetection"] == null } }

        assertEquals(
            WriteOutcome.Rejected(Value.BoolValue(false)),
            manager.settingWriteErrors.value["wearingDetection"],
        )
    }
}
