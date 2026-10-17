package com.oppzippy.openscq30.ui

import com.oppzippy.openscq30.features.soundcoredevice.service.ConnectionStatus
import com.oppzippy.openscq30.features.soundcoredevice.service.DeviceService
import com.oppzippy.openscq30.test.MainDispatcherRule
import io.mockk.every
import io.mockk.impl.annotations.MockK
import io.mockk.junit4.MockKRule
import io.mockk.mockk
import io.mockk.verify
import junit.framework.TestCase.assertEquals
import kotlin.time.Duration.Companion.milliseconds
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.timeout
import kotlinx.coroutines.test.runTest
import org.junit.Before
import org.junit.Rule
import org.junit.Test

@OptIn(FlowPreview::class)
class DeviceServiceConnectionTest {
    @get:Rule
    val mockkRule = MockKRule(this)

    @get:Rule
    val mainDispatcherRule = MainDispatcherRule()

    @MockK
    lateinit var binder: DeviceService.MyBinder

    @MockK
    lateinit var service: DeviceService

    private lateinit var connectionStatusFlow: MutableStateFlow<ConnectionStatus>

    @Before
    fun setUp() {
        connectionStatusFlow = MutableStateFlow(ConnectionStatus.Disconnected)
        every { service.connectionStatusFlow } returns connectionStatusFlow
    }

    @Test
    fun startsDisconnected() {
        val connection = DeviceServiceConnection()
        assertEquals(ConnectionStatus.Disconnected, connection.connectionStatusFlow.value)
    }

    @Test
    fun movesToLoadingState() = runTest {
        val connection = DeviceServiceConnection()

        every { binder.getService() } returns service
        connection.onServiceConnected(null, binder)
        connectionStatusFlow.value = ConnectionStatus.Connecting("00:00:00:00:00:00")

        // It will throw an exception if it times out, so no need to assert
        connection.connectionStatusFlow.timeout(10.milliseconds).first { it is ConnectionStatus.Connecting }
        connection.close()
    }

    @Test
    fun closingBindingStopsFollowingServiceWithoutStoppingService() = runTest {
        val connection = DeviceServiceConnection()
        every { binder.getService() } returns service
        connection.onServiceConnected(null, binder)
        connectionStatusFlow.value = ConnectionStatus.Connecting("first")

        connection.close()
        connectionStatusFlow.value = ConnectionStatus.Connecting("second")

        assertEquals(ConnectionStatus.Disconnected, connection.connectionStatusFlow.value)
        verify(exactly = 0) { service.stopSelf() }
    }

    @Test
    fun reconnectingBindingCancelsCollectorOfPreviousService() = runTest {
        val connection = DeviceServiceConnection()
        every { binder.getService() } returns service
        connection.onServiceConnected(null, binder)
        val previousFlow = connectionStatusFlow
        connectionStatusFlow = MutableStateFlow(ConnectionStatus.Connecting("new"))
        every { service.connectionStatusFlow } returns connectionStatusFlow

        connection.onServiceConnected(null, binder)
        previousFlow.value = ConnectionStatus.Connecting("stale")

        assertEquals("new", (connection.connectionStatusFlow.value as ConnectionStatus.Connecting).macAddress)
        connection.close()
    }

    @Test
    fun bindingFailuresNotifyOwnerAndReleaseCollectors() = runTest {
        var lostBindings = 0
        val connection = DeviceServiceConnection { lostBindings += 1 }
        every { binder.getService() } returns service
        connection.onServiceConnected(null, binder)

        connection.onBindingDied(null)
        connectionStatusFlow.value = ConnectionStatus.Connecting("stale")
        assertEquals(ConnectionStatus.Disconnected, connection.connectionStatusFlow.value)
        assertEquals(1, lostBindings)

        connection.onNullBinding(null)
        assertEquals(2, lostBindings)
    }
}
