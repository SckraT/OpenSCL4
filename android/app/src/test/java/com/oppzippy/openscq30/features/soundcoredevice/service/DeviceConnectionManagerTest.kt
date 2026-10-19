package com.oppzippy.openscq30.features.soundcoredevice.service

import com.oppzippy.openscq30.lib.bindings.AndroidRfcommConnectionBackend
import com.oppzippy.openscq30.lib.bindings.ManualConnectionBackends
import com.oppzippy.openscq30.lib.bindings.ManualRfcommConnectionBox
import com.oppzippy.openscq30.lib.bindings.RfcommServiceSelectionStrategy
import com.oppzippy.openscq30.lib.bindings.newSessionWithInMemoryDb
import com.oppzippy.openscq30.lib.wrapper.ConnectionDescriptor
import com.oppzippy.openscq30.lib.wrapper.PairedDevice
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.encodeToJsonElement
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class DeviceConnectionManagerTest {
    @Test(timeout = 15_000)
    fun nativeActivationUpdatesTheConnectionAcrossHandlersAndStartsOverOnReconnect() = runBlocking {
        val unusedBackend = object : AndroidRfcommConnectionBackend {
            override suspend fun devices(): List<ConnectionDescriptor> = error("demo uses no Bluetooth backend")

            override suspend fun connect(
                macAddress: String,
                serviceSelectionStrategy: RfcommServiceSelectionStrategy,
                outputBox: ManualRfcommConnectionBox,
            ) = error("demo uses no Bluetooth backend")
        }
        newSessionWithInMemoryDb().use { session ->
            ManualConnectionBackends(unusedBackend).use { backends ->
                val model = "SoundcoreA3954"
                val address = session.listDemoDevices(model).first().macAddress
                session.pair(PairedDevice(address, model, true))
                DeviceConnectionManager(session.connectWithBackends(backends, address)).use { connection ->
                    assertNull(connection.lastAppliedPreset.value)
                    val snapshot = connection.device.settingsByCategory()
                    assertEquals(connection.device.categories(), snapshot.map { it.categoryId })
                    snapshot.forEach { category ->
                        val expected = connection.device.settingsInCategory(category.categoryId).mapNotNull { id ->
                            connection.device.setting(id)?.let { id to Json.encodeToJsonElement(it) }
                        }
                        assertEquals(expected, category.settings.map { it.settingId to Json.encodeToJsonElement(it.setting) })
                    }
                    session.quickPresetHandler().use { handler ->
                        handler.save(connection.device, "First")
                        handler.toggleField(connection.device, "First", "wearingDetection", true)
                        handler.save(connection.device, "Second")
                        handler.toggleField(connection.device, "Second", "wearingDetection", true)
                        handler.activate(connection.device, "First")
                    }
                    withTimeout(5_000) { connection.lastAppliedPreset.first { it == "First" } }
                    session.quickPresetHandler().use { notificationHandler ->
                        assertTrue(runCatching { notificationHandler.activate(connection.device, "Missing") }.isFailure)
                        assertEquals("First", connection.device.lastAppliedPreset())
                        notificationHandler.activate(connection.device, "Second")
                    }
                    withTimeout(5_000) { connection.lastAppliedPreset.first { it == "Second" } }
                }
                DeviceConnectionManager(session.connectWithBackends(backends, address)).use { connection ->
                    assertNull(connection.lastAppliedPreset.value)
                    session.quickPresetHandler().use { handler ->
                        assertEquals(2, handler.quickPresets(connection.device).size)
                    }
                }
            }
        }
    }
}
