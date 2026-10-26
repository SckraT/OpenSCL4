package com.oppzippy.openscq30.ui

import android.util.Log
import com.oppzippy.openscq30.features.equalizer.storage.LegacyEqualizerProfile
import com.oppzippy.openscq30.features.soundcoredevice.service.DeviceConnectionManager
import com.oppzippy.openscq30.lib.bindings.AndroidRfcommConnectionBackend
import com.oppzippy.openscq30.lib.bindings.ManualConnectionBackends
import com.oppzippy.openscq30.lib.bindings.ManualRfcommConnectionBox
import com.oppzippy.openscq30.lib.bindings.RfcommServiceSelectionStrategy
import com.oppzippy.openscq30.lib.bindings.SettingIdValuePair
import com.oppzippy.openscq30.lib.bindings.newSessionWithInMemoryDb
import com.oppzippy.openscq30.lib.wrapper.ConnectionDescriptor
import com.oppzippy.openscq30.lib.wrapper.PairedDevice
import com.oppzippy.openscq30.lib.wrapper.Setting
import com.oppzippy.openscq30.lib.wrapper.toValue
import io.mockk.every
import io.mockk.mockk
import io.mockk.mockkStatic
import io.mockk.unmockkStatic
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test

class LegacyEqualizerProfileMigrationTest {
    private val unusedBackend = object : AndroidRfcommConnectionBackend {
        override suspend fun devices(): List<ConnectionDescriptor> = error("demo uses no Bluetooth backend")

        override suspend fun connect(
            macAddress: String,
            serviceSelectionStrategy: RfcommServiceSelectionStrategy,
            outputBox: ManualRfcommConnectionBox,
        ) = error("demo uses no Bluetooth backend")
    }

    @Before
    fun setUp() {
        mockkStatic(Log::class)
        every { Log.v(any(), any()) } returns 0
        every { Log.w(any(), any<String>(), any()) } returns 0
        every { Log.e(any(), any<String>(), any()) } returns 0
    }

    @After
    fun tearDown() {
        unmockkStatic(Log::class)
    }

    @Test(timeout = 15_000)
    fun aMigratedProfileKeepsItsOwnValuesAndTheSoundStaysTheSame() = runBlocking {
        newSessionWithInMemoryDb().use { session ->
            ManualConnectionBackends(unusedBackend).use { backends ->
                val model = "SoundcoreA3028"
                val address = session.listDemoDevices(model).first().macAddress
                session.pair(PairedDevice(address, model, true))
                DeviceConnectionManager(session.connectWithBackends(backends, address)).use { connection ->
                    val device = connection.device
                    fun volumeAdjustments() = (device.setting("volumeAdjustments") as Setting.EqualizerSetting).value
                    fun savedNames() =
                        (device.setting("customEqualizerProfile") as Setting.ModifiableSelectSetting).setting.options
                    val soundBefore = volumeAdjustments()
                    val manager = DeviceSettingsManager(
                        session = session,
                        parent = Job(),
                        deviceManager = connection,
                        quickPresetSlotDao = mockk(relaxed = true),
                        featuredSettingSlotDao = mockk(relaxed = true),
                        legacyEqualizerProfileDao = mockk(relaxed = true),
                        toastHandler = mockk(relaxed = true),
                    )

                    manager.use {
                        it.migrateLegacyEqualizerProfiles(
                            listOf(LegacyEqualizerProfile("Old", 10, -20, 30, 40, 50, 60, 70, 80)),
                        )
                        withTimeout(5_000) {
                            while ("Old" !in savedNames()) delay(10)
                        }
                    }

                    assertEquals(soundBefore, volumeAdjustments())
                    device.setSettingValues(listOf(SettingIdValuePair("customEqualizerProfile", "Old".toValue())))
                    assertEquals(listOf<Short>(10, -20, 30, 40, 50, 60, 70, 80), volumeAdjustments())
                }
            }
        }
    }
}
