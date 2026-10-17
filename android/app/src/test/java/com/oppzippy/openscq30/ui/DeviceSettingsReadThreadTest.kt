package com.oppzippy.openscq30.ui

import com.oppzippy.openscq30.features.soundcoredevice.service.DeviceConnectionManager
import com.oppzippy.openscq30.lib.bindings.OpenScq30Device
import com.oppzippy.openscq30.lib.bindings.OpenScq30Session
import com.oppzippy.openscq30.ui.utils.ToastHandler
import io.mockk.coEvery
import io.mockk.every
import io.mockk.mockk
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Job
import kotlinx.coroutines.asCoroutineDispatcher
import kotlinx.coroutines.async
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeout
import org.junit.Assert.assertTrue
import org.junit.Test

class DeviceSettingsReadThreadTest {
    @Test
    fun nativeReadsKeepTheCollectingUiThreadResponsive(): Unit = runBlocking {
        Executors.newSingleThreadExecutor().asCoroutineDispatcher().use { uiDispatcher ->
            // A slow synchronous native call must not prevent the UI from handling another event.
            for (read in listOf("categories", "categorySettings", "allSettings")) {
                val entered = CompletableDeferred<Unit>()
                val release = CountDownLatch(1)
                val device = mockk<OpenScq30Device>(relaxed = true)
                fun slowRead(): List<String> {
                    entered.complete(Unit)
                    check(release.await(10, TimeUnit.SECONDS))
                    return emptyList()
                }
                every { device.categories() } answers { slowRead() }
                every { device.settingsInCategory(any()) } answers { slowRead() }
                val session = mockk<OpenScq30Session>(relaxed = true)
                coEvery { session.quickPresetHandler().quickPresets(any()) } returns emptyList()
                val parent = Job()
                val manager = DeviceSettingsManager(
                    session = session,
                    parent = parent,
                    deviceManager = DeviceConnectionManager(device),
                    quickPresetSlotDao = mockk(relaxed = true),
                    featuredSettingSlotDao = mockk(relaxed = true),
                    legacyEqualizerProfileDao = mockk(relaxed = true),
                    toastHandler = ToastHandler(),
                )
                val query = async(uiDispatcher) {
                    when (read) {
                        "categories" -> manager.categoryIdsFlow.first()
                        "categorySettings" -> manager.getSettingsInCategoryFlow("soundModes").first()
                        else -> manager.allSettingsFlow.first()
                    }
                }
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
    }
}
