package com.oppzippy.openscq30.features.autoconnect

import android.app.BackgroundServiceStartNotAllowedException
import android.app.ForegroundServiceStartNotAllowedException
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.util.Log
import androidx.core.content.ContextCompat
import com.oppzippy.openscq30.features.preferences.Preferences
import io.mockk.every
import io.mockk.mockk
import io.mockk.mockkObject
import io.mockk.mockkStatic
import io.mockk.unmockkObject
import io.mockk.unmockkStatic
import io.mockk.verify
import org.junit.After
import org.junit.Assert.assertFalse
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

class AutoConnectStartTest {
    private val context: Context = mockk(relaxed = true)

    @Before
    fun setUp() {
        mockkStatic(Log::class)
        every { Log.d(any<String>(), any<String>()) } returns 0
        every { Log.w(any<String>(), any<String>()) } returns 0
        every { Log.w(any<String>(), any<String>(), any<Throwable>()) } returns 0
        // The real helper reaches into android.jar stubs, which throw here instead of answering.
        mockkStatic(ContextCompat::class)
        every { ContextCompat.checkSelfPermission(any(), any()) } returns PackageManager.PERMISSION_GRANTED
    }

    @After
    fun tearDown() {
        unmockkStatic(Log::class)
        unmockkStatic(ContextCompat::class)
    }

    private fun preferences(autoConnect: Boolean): Preferences {
        val preferences = mockk<Preferences>(relaxed = true)
        every { preferences.autoConnect } returns autoConnect
        return preferences
    }

    @Test
    fun enabledAutoconnectStartsTheForegroundService() {
        startAutoConnectServiceIfEnabled(context, preferences(autoConnect = true))

        verify(exactly = 1) { context.startForegroundService(any<Intent>()) }
    }

    @Test
    fun disabledAutoconnectDoesNotTouchTheService() {
        startAutoConnectServiceIfEnabled(context, preferences(autoConnect = false))

        verify(exactly = 0) { context.startForegroundService(any<Intent>()) }
    }

    @Test
    fun missingNotificationPermissionDoesNotStartTheService() {
        denyNotificationPermission()

        startAutoConnectServiceIfEnabled(context, preferences(autoConnect = true), sdkInt = 33)

        verify(exactly = 0) { context.startForegroundService(any<Intent>()) }
    }

    @Test
    fun aRunningServiceIsNotStartedAgain() {
        // Starting it again would post the notification even while a device is connected.
        mockkObject(AutoConnectService.Companion)
        try {
            every { AutoConnectService.isRunning } returns true

            startAutoConnectServiceIfEnabled(context, preferences(autoConnect = true))
        } finally {
            unmockkObject(AutoConnectService.Companion)
        }

        verify(exactly = 0) { context.startForegroundService(any<Intent>()) }
    }

    @Test
    fun autoconnectWithoutNotificationPermissionIsTurnedOff() {
        denyNotificationPermission()
        val preferences = preferences(autoConnect = true)

        assertTrue(turnOffAutoConnectWithoutNotifications(context, preferences, sdkInt = 33))

        verify(exactly = 1) { preferences.autoConnect = false }
        verify(exactly = 1) { context.stopService(any<Intent>()) }
    }

    @Test
    fun autoconnectWithNotificationPermissionStaysOn() {
        val preferences = preferences(autoConnect = true)

        assertFalse(turnOffAutoConnectWithoutNotifications(context, preferences, sdkInt = 33))

        verify(exactly = 0) { preferences.autoConnect = any() }
        verify(exactly = 0) { context.stopService(any<Intent>()) }
    }

    @Test
    fun autoconnectBeforeAndroid13NeedsNoNotificationPermissionToStayOn() {
        denyNotificationPermission()

        assertFalse(turnOffAutoConnectWithoutNotifications(context, preferences(autoConnect = true), sdkInt = 32))
    }

    @Test
    fun disabledAutoconnectIsLeftAloneWithoutNotificationPermission() {
        denyNotificationPermission()

        assertFalse(turnOffAutoConnectWithoutNotifications(context, preferences(autoConnect = false), sdkInt = 33))

        verify(exactly = 0) { context.stopService(any<Intent>()) }
    }

    @Test
    fun refusedForegroundStartIsReportedInsteadOfThrown() {
        every { context.startForegroundService(any<Intent>()) } throws
            IllegalStateException("Not allowed to start service")

        // Must not propagate: the process has to survive the refused start.
        startAutoConnectServiceIfEnabled(context, preferences(autoConnect = true))

        verify(exactly = 1) { context.startForegroundService(any<Intent>()) }
    }

    @Test
    fun otherStartFailuresStillThrow() {
        every { context.startForegroundService(any<Intent>()) } throws SecurityException("missing permission")

        assertThrows(SecurityException::class.java) {
            startAutoConnectServiceIfEnabled(context, preferences(autoConnect = true))
        }
    }

    @Test
    fun aRefusedForegroundStartIsRecognisedFromAndroid12() {
        assertTrue(isServiceStartRefusal(mockk<ForegroundServiceStartNotAllowedException>(), sdkInt = 31))
    }

    @Test
    fun aRefusedBackgroundStartIsStillRecognisedFromAndroid12() {
        assertTrue(isServiceStartRefusal(mockk<BackgroundServiceStartNotAllowedException>(), sdkInt = 31))
    }

    @Test
    fun otherIllegalStateFailuresAreNotMistakenForARefusalFromAndroid12() {
        assertFalse(isServiceStartRefusal(IllegalStateException("boom"), sdkInt = 31))
    }

    @Test
    fun plainIllegalStateIsTheRefusalBeforeAndroid12() {
        // Android 8-11 have no ServiceStartNotAllowedException; the refusal is the plain base class.
        assertTrue(isServiceStartRefusal(IllegalStateException("Not allowed to start service"), sdkInt = 30))
    }

    @Test
    fun subclassedIllegalStateIsStillARefusalBeforeAndroid12() {
        // A ROM that subclasses IllegalStateException for the same refusal must not crash the process again.
        assertTrue(isServiceStartRefusal(SubclassedIllegalState("Not allowed to start service"), sdkInt = 30))
    }

    @Test
    fun presenceObservationIsSkippedBeforeAndroid12() {
        observeCompanionPresenceForAutoconnect(context)

        verify(exactly = 0) { context.getSystemService(any<Class<*>>()) }
    }

    private fun denyNotificationPermission() {
        every { ContextCompat.checkSelfPermission(any(), any()) } returns PackageManager.PERMISSION_DENIED
    }

    private class SubclassedIllegalState(message: String) : IllegalStateException(message)
}
