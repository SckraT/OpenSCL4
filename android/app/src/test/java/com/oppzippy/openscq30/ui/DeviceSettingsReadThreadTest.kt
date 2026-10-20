package com.oppzippy.openscq30.ui

import com.oppzippy.openscq30.features.soundcoredevice.service.DeviceConnectionManager
import com.oppzippy.openscq30.lib.bindings.OpenScq30Device
import com.oppzippy.openscq30.lib.bindings.OpenScq30Session
import com.oppzippy.openscq30.lib.bindings.SettingCategory
import com.oppzippy.openscq30.lib.bindings.SettingIdSettingPair
import com.oppzippy.openscq30.lib.wrapper.Setting
import com.oppzippy.openscq30.ui.utils.ToastHandler
import io.mockk.coEvery
import io.mockk.every
import io.mockk.mockk
import io.mockk.verify
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Job
import kotlinx.coroutines.asCoroutineDispatcher
import kotlinx.coroutines.async
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.onEach
import kotlinx.coroutines.flow.take
import kotlinx.coroutines.flow.toList
import kotlinx.coroutines.joinAll
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeout
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class DeviceSettingsReadThreadTest {
    private fun manager(device: OpenScq30Device, parent: Job, changes: MutableStateFlow<Int>): DeviceSettingsManager {
        val connection = mockk<DeviceConnectionManager>()
        every { connection.device } returns device
        every { connection.watchForChangeNotification } returns changes
        val session = mockk<OpenScq30Session>(relaxed = true)
        coEvery { session.quickPresetHandler().quickPresets(any()) } returns emptyList()
        return DeviceSettingsManager(
            session = session,
            parent = parent,
            deviceManager = connection,
            quickPresetSlotDao = mockk(relaxed = true),
            featuredSettingSlotDao = mockk(relaxed = true),
            legacyEqualizerProfileDao = mockk(relaxed = true),
            toastHandler = ToastHandler(),
        )
    }

    @Test
    fun nativeReadsKeepTheCollectingUiThreadResponsive(): Unit = runBlocking {
        Executors.newSingleThreadExecutor().asCoroutineDispatcher().use { uiDispatcher ->
            val entered = CompletableDeferred<Unit>()
            val release = CountDownLatch(1)
            val device = mockk<OpenScq30Device>(relaxed = true)
            every { device.settingsByCategory() } answers {
                entered.complete(Unit)
                check(release.await(10, TimeUnit.SECONDS))
                emptyList()
            }
            val parent = Job()
            val manager = manager(device, parent, MutableStateFlow(0))
            val query = async(uiDispatcher) { manager.allSettingsFlow.first() }
            try {
                withTimeout(5_000) { entered.await() }
                assertTrue(withTimeout(2_000) { withContext(uiDispatcher) { true } })
                release.countDown()
                assertTrue(withTimeout(5_000) { query.await() }.isEmpty())
            } finally {
                release.countDown()
                query.cancel()
                manager.close()
                parent.cancel()
            }
        }
    }

    @Test
    fun subscribersShareOneNativeReadPerChange(): Unit = runBlocking {
        val device = mockk<OpenScq30Device>(relaxed = true)
        val reads = AtomicInteger()
        val entered = CompletableDeferred<Unit>()
        val release = CountDownLatch(1)
        every { device.settingsByCategory() } answers {
            val count = reads.incrementAndGet()
            if (count == 1) {
                entered.complete(Unit)
                check(release.await(10, TimeUnit.SECONDS))
            }
            val field = SettingIdSettingPair("wearingDetection", Setting.ToggleSetting(count > 1))
            listOf(
                SettingCategory("general", listOf(field)),
                SettingCategory("equalizer", emptyList()),
                SettingCategory("deviceInformation", listOf(field)),
            )
        }
        val parent = Job()
        val changes = MutableStateFlow(0)
        val manager = manager(device, parent, changes)
        val firstSnapshot = CompletableDeferred<List<SettingCategory>>()
        val firstAll = CompletableDeferred<Unit>()
        val snapshots = async {
            manager.settingsByCategoryFlow.filterNotNull().onEach { firstSnapshot.complete(it) }.take(2).toList()
        }
        val all = async { manager.allSettingsFlow.onEach { firstAll.complete(Unit) }.take(2).toList() }
        try {
            withTimeout(5_000) { entered.await() }
            release.countDown()
            val first = withTimeout(5_000) { firstSnapshot.await() }
            withTimeout(5_000) { firstAll.await() }
            assertEquals(listOf("general", "equalizer", "deviceInformation"), first.map { it.categoryId })
            changes.value = 1
            val observed = withTimeout(5_000) { snapshots.await() }
            val values = withTimeout(5_000) { all.await() }
            assertEquals(2, observed.size)
            assertEquals(
                listOf(listOf("wearingDetection"), listOf("wearingDetection")),
                values.map { row ->
                    row.map { it.first }
                },
            )
            assertEquals(Setting.ToggleSetting(false), values[0][0].second)
            assertEquals(Setting.ToggleSetting(true), values[1][0].second)
            assertEquals(2, reads.get())
            verify(exactly = 0) { device.categories() }
            verify(exactly = 0) { device.settingsInCategory(any()) }
            verify(exactly = 0) { device.setting(any()) }
        } finally {
            release.countDown()
            snapshots.cancel()
            all.cancel()
            manager.close()
            parent.cancel()
        }
    }

    @Test
    fun closedConnectionDoesNotPublishItsLateRead(): Unit = runBlocking {
        val device = mockk<OpenScq30Device>(relaxed = true)
        val entered = CompletableDeferred<Unit>()
        val release = CountDownLatch(1)
        every { device.settingsByCategory() } answers {
            entered.complete(Unit)
            check(release.await(10, TimeUnit.SECONDS))
            listOf(SettingCategory("oldConnection", emptyList()))
        }
        val parent = Job()
        val old = manager(device, parent, MutableStateFlow(0))
        val query = async { old.settingsByCategoryFlow.filterNotNull().first() }
        try {
            withTimeout(5_000) { entered.await() }
            val jobs = parent.children.toList()
            old.close()
            release.countDown()
            withTimeout(5_000) { jobs.joinAll() }
            assertNull(old.settingsByCategoryFlow.value)
            val fresh = manager(mockk(relaxed = true), parent, MutableStateFlow(0))
            try {
                assertNull(fresh.settingsByCategoryFlow.value)
                assertTrue(withTimeout(5_000) { fresh.allSettingsFlow.first() }.isEmpty())
            } finally {
                fresh.close()
            }
        } finally {
            release.countDown()
            query.cancel()
            old.close()
            parent.cancel()
        }
    }
}
