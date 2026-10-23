package com.oppzippy.openscq30.features.soundcoredevice.service

import com.oppzippy.openscq30.features.autoconnect.AutoConnectCoordinator
import com.oppzippy.openscq30.lib.bindings.SessionFailureReason
import com.oppzippy.openscq30.lib.wrapper.PairedDevice
import io.mockk.every
import io.mockk.mockk
import io.mockk.verify
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.awaitCancellation
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.withContext
import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class DeviceConnectionControllerTest {
    private fun mockManager(
        sessionStatusFlow: MutableStateFlow<SessionConnectionState> =
            MutableStateFlow(SessionConnectionState.Connected),
    ): DeviceConnectionManager = mockk<DeviceConnectionManager>(relaxed = true).also {
        every { it.sessionStatusFlow } returns sessionStatusFlow
    }

    @Test
    fun reconcileThenBroadcastForTheSameDeviceConnectsOnce() = runTest {
        val manager = mockk<DeviceConnectionManager>(relaxed = true)
        var attempts = 0
        val controller = DeviceConnectionController(this, {
            attempts += 1
            manager
        }, { throw AssertionError(it) })
        val coordinator = AutoConnectCoordinator(
            isAutoConnectEnabled = { true },
            associatedAddresses = { listOf("AA:BB:CC:DD:EE:FF") },
            savedDevices = { listOf(PairedDevice("AA:BB:CC:DD:EE:FF", "model", false)) },
            connect = controller::connect,
        )

        coordinator.reconcileConnectedDevices { listOf("AA:BB:CC:DD:EE:FF") }
        runCurrent()
        coordinator.handleConnectedDevice("aa:bb:cc:dd:ee:ff")
        runCurrent()

        assertEquals(1, attempts)
        assertSame(
            manager,
            (controller.connectionStatusFlow.value as ConnectionStatus.Connected).deviceManager,
        )
        controller.close()
    }

    @Test
    fun restartedServiceConnectsTheSameDeviceAgain() = runTest {
        val manager = mockk<DeviceConnectionManager>(relaxed = true)
        var attempts = 0
        val beforeRestart = DeviceConnectionController(this, {
            attempts += 1
            manager
        }, { throw AssertionError(it) })
        beforeRestart.connect("AA:BB:CC:DD:EE:FF")
        runCurrent()
        beforeRestart.close()

        val afterRestart = DeviceConnectionController(this, {
            attempts += 1
            manager
        }, { throw AssertionError(it) })
        afterRestart.connect("AA:BB:CC:DD:EE:FF")
        runCurrent()

        assertEquals(2, attempts)
        assertSame(
            manager,
            (afterRestart.connectionStatusFlow.value as ConnectionStatus.Connected).deviceManager,
        )
        afterRestart.close()
    }

    @Test
    fun recoveringKeepsTheManagerAndDoesNotOpenAnotherSession() = runTest {
        val session = MutableStateFlow<SessionConnectionState>(SessionConnectionState.Connected)
        val manager = mockManager(session)
        var attempts = 0
        val controller = DeviceConnectionController(this, {
            attempts += 1
            manager
        }, { throw AssertionError(it) })

        controller.connect("device")
        runCurrent()
        session.value = SessionConnectionState.Recovering(attempt = 1, reason = SessionFailureReason.TRANSPORT_LOST)
        runCurrent()

        val recovering = controller.connectionStatusFlow.value as ConnectionStatus.Recovering
        assertSame(manager, recovering.deviceManager)
        assertEquals(1, recovering.attempt)
        assertEquals(SessionFailureReason.TRANSPORT_LOST, recovering.reason)

        controller.connect("device")
        runCurrent()
        assertEquals(1, attempts)
        verify(exactly = 0) { manager.close() }
        controller.close()
    }

    @Test
    fun retryWhileRecoveringDoesNotStartASecondSession() = runTest {
        val session = MutableStateFlow<SessionConnectionState>(SessionConnectionState.Connected)
        val manager = mockManager(session)
        var attempts = 0
        val controller = DeviceConnectionController(this, {
            attempts += 1
            manager
        }, { throw AssertionError(it) })

        controller.connect("device")
        runCurrent()
        session.value = SessionConnectionState.Recovering(attempt = 1, reason = SessionFailureReason.TRANSPORT_LOST)
        runCurrent()
        controller.retry()
        runCurrent()

        assertEquals(1, attempts)
        assertTrue(controller.connectionStatusFlow.value is ConnectionStatus.Recovering)
        verify(exactly = 0) { manager.close() }
        controller.close()
    }

    @Test
    fun lostKeepsTheManagerAndRetryStartsANewSession() = runTest {
        val firstSession = MutableStateFlow<SessionConnectionState>(SessionConnectionState.Connected)
        val first = mockManager(firstSession)
        val second = mockManager()
        var attempts = 0
        val controller = DeviceConnectionController(this, {
            attempts += 1
            if (attempts == 1) first else second
        }, { throw AssertionError(it) })

        controller.connect("device")
        runCurrent()
        firstSession.value = SessionConnectionState.Lost(SessionFailureReason.CONNECT_TIMED_OUT)
        runCurrent()

        val lost = controller.connectionStatusFlow.value as ConnectionStatus.Lost
        assertSame(first, lost.deviceManager)
        assertEquals(SessionFailureReason.CONNECT_TIMED_OUT, lost.reason)

        controller.retry()
        runCurrent()

        assertEquals(2, attempts)
        verify(exactly = 1) { first.close() }
        verify(exactly = 0) { second.close() }
        assertSame(second, (controller.connectionStatusFlow.value as ConnectionStatus.Connected).deviceManager)
        controller.close()
    }

    @Test
    fun aLinkEventStartsANewBudgetOnlyForTheMatchingLostDevice() = runTest {
        val firstSession = MutableStateFlow<SessionConnectionState>(SessionConnectionState.Connected)
        val first = mockManager(firstSession)
        val second = mockManager()
        var attempts = 0
        val controller = DeviceConnectionController(this, {
            attempts += 1
            if (attempts == 1) first else second
        }, { throw AssertionError(it) })

        controller.connect("AA:BB:CC:DD:EE:FF")
        runCurrent()
        firstSession.value = SessionConnectionState.Lost(SessionFailureReason.CONNECT_TIMED_OUT)
        runCurrent()

        controller.retryIfLostFor("00:11:22:33:44:55")
        runCurrent()
        assertEquals(1, attempts)

        controller.retryIfLostFor("aa:bb:cc:dd:ee:ff")
        runCurrent()
        assertEquals(2, attempts)
        assertSame(second, (controller.connectionStatusFlow.value as ConnectionStatus.Connected).deviceManager)
        controller.close()
    }

    @Test
    fun repeatedSelectionWhileConnectingStartsOnlyOneConnection() = runTest {
        var attempts = 0
        val controller = DeviceConnectionController(this, {
            attempts += 1
            awaitCancellation()
        }, { throw AssertionError(it) })

        controller.connect("AA:BB:CC:DD:EE:FF")
        runCurrent()
        controller.connect("aa:bb:cc:dd:ee:ff")
        runCurrent()

        assertEquals(1, attempts)
        controller.close()
    }

    @Test
    fun repeatedSelectionOfConnectedDeviceKeepsNativeManager() = runTest {
        val manager = mockManager()
        var attempts = 0
        val controller = DeviceConnectionController(this, {
            attempts += 1
            manager
        }, { throw AssertionError(it) })

        controller.connect("AA:BB:CC:DD:EE:FF")
        runCurrent()
        controller.connect("aa:bb:cc:dd:ee:ff")
        runCurrent()

        assertEquals(1, attempts)
        assertSame(manager, (controller.connectionStatusFlow.value as ConnectionStatus.Connected).deviceManager)
        verify(exactly = 0) { manager.close() }
        controller.close()
    }

    @Test
    fun switchingConnectedDevicesClosesPreviousManager() = runTest {
        val first = mockManager()
        val second = mockManager()
        val controller = DeviceConnectionController(this, { address ->
            if (address == "first") first else second
        }, { throw AssertionError(it) })

        controller.connect("first")
        runCurrent()
        controller.connect("second")
        runCurrent()

        verify(exactly = 1) { first.close() }
        verify(exactly = 0) { second.close() }
        assertSame(second, (controller.connectionStatusFlow.value as ConnectionStatus.Connected).deviceManager)
        controller.close()
        controller.close()
        verify(exactly = 1) { second.close() }
    }

    @Test
    fun switchingCancelsPendingConnectionAndDisposesLateNativeResult() = runTest {
        val lateResult = mockManager()
        val second = mockManager()
        val releaseFirst = CompletableDeferred<Unit>()
        var secondStarted = false
        val controller = DeviceConnectionController(this, { address ->
            if (address == "first") {
                // Model a native call that returns its result despite coroutine cancellation.
                withContext(NonCancellable) { releaseFirst.await() }
                lateResult
            } else {
                secondStarted = true
                second
            }
        }, { throw AssertionError(it) })

        controller.connect("first")
        runCurrent()
        controller.connect("second")
        runCurrent()
        assertTrue(!secondStarted)
        releaseFirst.complete(Unit)
        runCurrent()

        assertTrue(secondStarted)
        verify(exactly = 1) { lateResult.close() }
        assertSame(second, (controller.connectionStatusFlow.value as ConnectionStatus.Connected).deviceManager)
        controller.close()
    }

    @Test
    fun failedConnectionCanBeRetriedAtSameAddress() = runTest {
        val failure = IllegalStateException("connection failed")
        val manager = mockManager()
        val failures = mutableListOf<Exception>()
        var attempts = 0
        val controller = DeviceConnectionController(this, {
            attempts += 1
            if (attempts == 1) throw failure
            manager
        }, { failures.add(it) })

        controller.connect("same")
        runCurrent()
        assertTrue(controller.connectionStatusFlow.value is ConnectionStatus.Lost)
        assertEquals(listOf(failure), failures)

        controller.connect("same")
        runCurrent()
        assertEquals(2, attempts)
        assertSame(manager, (controller.connectionStatusFlow.value as ConnectionStatus.Connected).deviceManager)
        controller.close()
    }

    @Test
    fun rapidSwitchingWaitsForOldNativeCallAndSkipsSupersededRequest() = runTest {
        val lateResult = mockManager()
        val latest = mockManager()
        val releaseFirst = CompletableDeferred<Unit>()
        val attempts = mutableListOf<String>()
        val controller = DeviceConnectionController(this, { address ->
            attempts.add(address)
            if (address == "first") {
                withContext(NonCancellable) { releaseFirst.await() }
                lateResult
            } else {
                latest
            }
        }, { throw AssertionError(it) })

        controller.connect("first")
        runCurrent()
        controller.connect("second")
        runCurrent()
        controller.connect("third")
        runCurrent()
        assertEquals(listOf("first"), attempts)

        releaseFirst.complete(Unit)
        runCurrent()
        assertEquals(listOf("first", "third"), attempts)
        verify(exactly = 1) { lateResult.close() }
        assertSame(latest, (controller.connectionStatusFlow.value as ConnectionStatus.Connected).deviceManager)
        controller.close()
    }

    @Test
    fun explicitDisconnectCancelsPendingConnection() = runTest {
        var cancelled = false
        val failures = mutableListOf<Exception>()
        val controller = DeviceConnectionController(this, {
            try {
                awaitCancellation()
            } finally {
                cancelled = true
            }
        }, { failures.add(it) })

        controller.connect("device")
        runCurrent()
        controller.close()
        runCurrent()

        assertTrue(cancelled)
        assertTrue(failures.isEmpty())
        assertSame(ConnectionStatus.Disconnected, controller.connectionStatusFlow.value)
    }

    @Test
    fun retryAfterExplicitDisconnectWaitsForNativeCleanup() = runTest {
        val lateResult = mockManager()
        val latest = mockManager()
        val releaseFirst = CompletableDeferred<Unit>()
        val attempts = mutableListOf<String>()
        val controller = DeviceConnectionController(this, { address ->
            attempts.add(address)
            if (address == "first") {
                withContext(NonCancellable) { releaseFirst.await() }
                lateResult
            } else {
                latest
            }
        }, { throw AssertionError(it) })

        controller.connect("first")
        runCurrent()
        controller.close()
        controller.connect("retry")
        runCurrent()
        assertEquals(listOf("first"), attempts)

        releaseFirst.complete(Unit)
        runCurrent()
        assertEquals(listOf("first", "retry"), attempts)
        verify(exactly = 1) { lateResult.close() }
        assertSame(latest, (controller.connectionStatusFlow.value as ConnectionStatus.Connected).deviceManager)
        controller.close()
    }
}
