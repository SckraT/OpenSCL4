package com.oppzippy.openscq30.features.autoconnect

import android.app.BackgroundServiceStartNotAllowedException
import android.content.Context
import android.content.Intent
import android.util.Log
import com.oppzippy.openscq30.features.preferences.Preferences
import io.mockk.every
import io.mockk.mockk
import io.mockk.mockkStatic
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
        every { Log.w(any<String>(), any<String>(), any<Throwable>()) } returns 0
    }

    @After
    fun tearDown() {
        unmockkStatic(Log::class)
    }

    private fun preferences(autoConnect: Boolean): Preferences {
        val preferences = mockk<Preferences>(relaxed = true)
        every { preferences.autoConnect } returns autoConnect
        return preferences
    }

    @Test
    fun enabledAutoconnectStartsTheService() {
        startAutoConnectServiceIfEnabled(context, preferences(autoConnect = true))

        verify(exactly = 1) { context.startService(any<Intent>()) }
    }

    @Test
    fun disabledAutoconnectDoesNotTouchTheService() {
        startAutoConnectServiceIfEnabled(context, preferences(autoConnect = false))

        verify(exactly = 0) { context.startService(any<Intent>()) }
    }

    @Test
    fun backgroundStartRefusalIsReportedInsteadOfThrown() {
        every { context.startService(any<Intent>()) } throws IllegalStateException("Not allowed to start service")

        // Must not propagate: the process has to survive the refused start.
        startAutoConnectServiceIfEnabled(context, preferences(autoConnect = true))

        verify(exactly = 1) { context.startService(any<Intent>()) }
    }

    @Test
    fun otherStartFailuresStillThrow() {
        every { context.startService(any<Intent>()) } throws SecurityException("missing permission")

        assertThrows(SecurityException::class.java) {
            startAutoConnectServiceIfEnabled(context, preferences(autoConnect = true))
        }
    }

    @Test
    fun backgroundStartRefusalIsRecognisedFromAndroid12() {
        val refusal = mockk<BackgroundServiceStartNotAllowedException>()

        assertTrue(isBackgroundStartRefusal(refusal, sdkInt = 31))
    }

    @Test
    fun otherIllegalStateFailuresAreNotMistakenForARefusalFromAndroid12() {
        assertFalse(isBackgroundStartRefusal(IllegalStateException("boom"), sdkInt = 31))
    }

    @Test
    fun plainIllegalStateIsTheRefusalBeforeAndroid12() {
        // Android 8-11 have no BackgroundServiceStartNotAllowedException; the refusal is the plain base class.
        assertTrue(isBackgroundStartRefusal(IllegalStateException("Not allowed to start service"), sdkInt = 30))
    }

    @Test
    fun subclassedIllegalStateIsStillARefusalBeforeAndroid12() {
        // A ROM that subclasses IllegalStateException for the same refusal must not crash the process again.
        assertTrue(isBackgroundStartRefusal(SubclassedIllegalState("Not allowed to start service"), sdkInt = 30))
    }

    private class SubclassedIllegalState(message: String) : IllegalStateException(message)
}
