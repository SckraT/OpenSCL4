package com.oppzippy.openscq30

import androidx.annotation.StringRes
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.state.ToggleableState
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertIsOff
import androidx.compose.ui.test.assertIsOn
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasScrollAction
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isDialog
import androidx.compose.ui.test.isSelectable
import androidx.compose.ui.test.isToggleable
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performScrollToNode
import androidx.compose.ui.test.performTextInput
import androidx.lifecycle.ViewModelProvider
import com.oppzippy.openscq30.actions.addAndConnectToDemoDevice
import com.oppzippy.openscq30.actions.awaitListItem
import com.oppzippy.openscq30.features.soundcoredevice.service.ConnectionStatus
import com.oppzippy.openscq30.features.soundcoredevice.service.DeviceConnectionManager
import com.oppzippy.openscq30.features.soundcoredevice.service.DeviceService
import com.oppzippy.openscq30.lib.bindings.translateCategoryId
import com.oppzippy.openscq30.lib.bindings.translateDeviceModel
import com.oppzippy.openscq30.lib.bindings.translateSettingId
import com.oppzippy.openscq30.lib.wrapper.Setting
import com.oppzippy.openscq30.ui.OpenSCQ30RootViewModel
import com.oppzippy.openscq30.ui.deviceselection.splitBrand
import dagger.hilt.android.testing.HiltAndroidTest
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/** Exercises the real service and native demo backend, rather than mocked navigation callbacks. */
@HiltAndroidTest
class SoundNavigationTest : OpenSCQ30RootTestBase() {
    private lateinit var rootViewModel: OpenSCQ30RootViewModel
    private val modelName get() = translateDeviceModel("SoundcoreA3954")

    // My devices shows the name without the brand
    private val cardName get() = splitBrand(modelName).second
    private val savedCard get() = hasClickAction() and hasText(cardName)
    private val presetName = "Sound UI navigation test"

    @Before
    fun setUp() {
        composeRule.runOnIdle {
            rootViewModel = ViewModelProvider(composeRule.activity)[OpenSCQ30RootViewModel::class.java]
        }
    }

    @After
    fun stopServiceBeforeNextActivity() {
        composeRule.runOnIdle { rootViewModel.stopDeviceService() }
        composeRule.waitUntil(uiTimeoutMillis()) { !DeviceService.isRunning.value }
    }

    @Test
    fun nestedAndRootBackPreserveConnectionAndReopenTheSameDevice() {
        val originalConnection = connectLiberty4Pro()

        waitForNode(hasText(getString(R.string.sound_overview_customize)))
        composeRule.onNodeWithText(getString(R.string.sound_overview_customize)).performScrollTo().performClick()
        waitForNode(hasText(getString(R.string.sound_device_modes_headline)))
        composeRule.onNode(hasText(getString(R.string.sound_overview_device)) and isSelectable())
            .assertDoesNotExist()

        pressBack()
        waitForNavigation(R.string.sound_overview_device)
        assertSame(originalConnection, connectedDevice())

        pressBack()
        waitForNode(hasText(getString(R.string.sound_my_devices)))
        assertTrue("Back must keep the foreground connection service running", DeviceService.isRunning.value)
        assertSame("Returning to the device list must retain the connection", originalConnection, connectedDevice())

        awaitListItem(composeRule, savedCard)
        composeRule.onNode(savedCard).performClick()
        waitForNavigation(R.string.sound_overview_device)
        assertSame("Opening the connected device must not reconnect it", originalConnection, connectedDevice())
    }

    @Test
    fun allSettingsAreGroupedByWhatTheyAreFor() {
        connectLiberty4Pro()
        composeRule.onNodeWithText(getString(R.string.sound_overview_all_settings)).performScrollTo().performClick()
        waitForNode(hasText(getString(R.string.sound_overview_effects)) and hasClickAction())
        composeRule.onNode(hasText(translateCategoryId("deviceInformation")) and hasClickAction()).assertExists()
        // nothing of the Liberty 4 Pro is left over, and the notification controls are in the app settings
        composeRule.onNodeWithText(translateCategoryId("miscellaneous")).assertDoesNotExist()
        composeRule.onNodeWithText(getString(R.string.sound_notification_controls)).assertDoesNotExist()
        composeRule.onNode(hasText(getString(R.string.sound_overview_power)) and hasClickAction())
            .performScrollTo().performClick()
        waitForNode(hasText(getString(R.string.sound_device_power_headline)))
        composeRule.onNodeWithText(translateSettingId("autoPowerOff")).assertExists()
    }

    @Test
    fun disconnectRequiresConfirmationAndStopsTheConnectionService() {
        val originalConnection = connectLiberty4Pro()
        openDisconnectConfirmation()
        composeRule.onNode(hasText(getString(R.string.cancel)) and hasAnyAncestor(isDialog())).performClick()
        assertSame(originalConnection, connectedDevice())

        openDisconnectConfirmation()
        composeRule.onNode(hasText(getString(R.string.disconnect)) and hasAnyAncestor(isDialog())).performClick()
        composeRule.waitUntil(uiTimeoutMillis()) {
            rootViewModel.connectionStatusFlow.value == ConnectionStatus.Disconnected && !DeviceService.isRunning.value
        }
        waitForNode(hasText(getString(R.string.sound_my_devices)))
        awaitListItem(composeRule, savedCard)
        composeRule.onNode(savedCard).assertExists("Disconnect must keep the saved device")
    }

    @Test
    fun settingsAndPresetsNavigateAndPresetRestoresADeviceSetting() {
        connectLiberty4Pro()

        selectNavigation(R.string.settings)
        waitForNode(hasText(getString(R.string.sound_settings_headline)))
        selectNavigation(R.string.sound_overview_device)

        openComfort()
        val initialWearingDetection = wearingDetection()
        wearingDetectionRow().performScrollTo().performClick()
        waitForWearingDetection(!initialWearingDetection)
        if (initialWearingDetection) wearingDetectionRow().assertIsOff() else wearingDetectionRow().assertIsOn()
        val savedWearingDetection = !initialWearingDetection
        pressBack()

        // The native test database can survive an instrumentation rerun. Remove only this test's preset.
        val deviceSettings = requireNotNull(rootViewModel.deviceSettingsManager.value)
        composeRule.runOnIdle { deviceSettings.deleteQuickPreset(presetName) }
        composeRule.waitUntil(uiTimeoutMillis()) {
            deviceSettings.quickPresetsFlow.value.none { it.name == presetName }
        }
        selectNavigation(R.string.quick_presets)
        composeRule.onNode(hasScrollAction())
            .performScrollToNode(hasText(getString(R.string.sound_preset_create)))
        composeRule.onNodeWithText(getString(R.string.sound_preset_create)).performClick()
        composeRule.onNodeWithText(getString(R.string.name)).performTextInput(presetName)
        composeRule.onNode(hasText(getString(R.string.create)) and hasAnyAncestor(isDialog())).performClick()
        composeRule.waitUntil(uiTimeoutMillis()) { deviceSettings.quickPresetsFlow.value.any { it.name == presetName } }
        presetButton(R.string.sound_preset_edit).performScrollTo().performClick()

        val wearingLabel = translateSettingId("wearingDetection")
        composeRule.onNodeWithTag("sound_preset_editor").performScrollToNode(hasText(wearingLabel))
        composeRule.onNode(hasText(wearingLabel) and isToggleable()).assertIsOff().performClick()
        composeRule.waitUntil(uiTimeoutMillis()) {
            deviceSettings.quickPresetsFlow.value.find { it.name == presetName }?.fields
                ?.any { it.settingId == "wearingDetection" && it.isEnabled } == true
        }
        pressBack()
        // the device has the new preset values, but only a preset applied from the list counts as applied
        presetButton(R.string.sound_preset_apply).assertIsEnabled()

        selectNavigation(R.string.sound_overview_device)
        openComfort()
        wearingDetectionRow().performScrollTo().performClick()
        waitForWearingDetection(!savedWearingDetection)
        pressBack()
        selectNavigation(R.string.quick_presets)
        presetButton(R.string.sound_preset_apply).assertIsEnabled().performClick()
        waitForWearingDetection(savedWearingDetection, waitForRow = false)
        presetButton(R.string.sound_preset_applied).assertIsNotEnabled()

        selectNavigation(R.string.sound_overview_device)
        openComfort()
        assertEquals(savedWearingDetection, wearingDetection())
        if (savedWearingDetection) wearingDetectionRow().assertIsOn() else wearingDetectionRow().assertIsOff()
    }

    @Test
    fun removingTheConnectedDeviceStopsControlAndRemovesItsSavedCard() {
        connectLiberty4Pro()
        val savedDevice = runBlocking {
            composeRule.activity.session.pairedDevices().first { it.model == "SoundcoreA3954" && it.isDemo }
        }
        try {
            pressBack()
            waitForNode(hasText(getString(R.string.sound_my_devices)))
            awaitListItem(composeRule, savedCard)
            composeRule.onNodeWithContentDescription(getString(R.string.sound_device_actions, modelName))
                .performScrollTo().performClick()
            composeRule.onNodeWithText(getString(R.string.unpair_device)).performClick()
            composeRule.onNode(hasText(getString(R.string.unpair)) and hasAnyAncestor(isDialog())).performClick()
            composeRule.waitUntil(uiTimeoutMillis()) { !DeviceService.isRunning.value }
            assertEquals(ConnectionStatus.Disconnected, rootViewModel.connectionStatusFlow.value)
            assertTrue(
                runBlocking {
                    composeRule.activity.session.pairedDevices().none { it.macAddress == savedDevice.macAddress }
                },
            )
            composeRule.waitUntil(uiTimeoutMillis()) {
                composeRule.onAllNodes(hasText(cardName)).fetchSemanticsNodes().isEmpty()
            }
            composeRule.onNodeWithText(cardName).assertDoesNotExist()
        } finally {
            // Restore the existing fixture for other tests and manual demo validation.
            runBlocking { composeRule.activity.session.pair(savedDevice) }
        }
    }

    private fun connectLiberty4Pro(): DeviceConnectionManager {
        waitForNode(hasText(getString(R.string.sound_my_devices)))
        addAndConnectToDemoDevice(composeRule, modelName)
        composeRule.waitUntil(uiTimeoutMillis()) {
            rootViewModel.connectionStatusFlow.value is ConnectionStatus.Connected &&
                rootViewModel.deviceSettingsManager.value != null
        }
        waitForNavigation(R.string.sound_overview_device)
        return connectedDevice()
    }

    private fun connectedDevice(): DeviceConnectionManager =
        (rootViewModel.connectionStatusFlow.value as ConnectionStatus.Connected).deviceManager

    private fun openDisconnectConfirmation() {
        composeRule.onNodeWithContentDescription(getString(R.string.sound_overview_device_actions)).performClick()
        composeRule.onNodeWithText(getString(R.string.sound_overview_disconnect)).performClick()
        waitForNode(hasText(getString(R.string.sound_overview_disconnect_confirm)))
    }

    private fun openComfort() {
        waitForNode(hasText(getString(R.string.sound_overview_comfort)))
        composeRule.onNodeWithText(getString(R.string.sound_overview_comfort)).performScrollTo().performClick()
        waitForNode(hasText(getString(R.string.sound_device_comfort_headline)))
    }

    private fun wearingDetection(): Boolean =
        (connectedDevice().device.setting("wearingDetection") as Setting.ToggleSetting).value

    private fun wearingDetectionRow() =
        composeRule.onNode(hasText(translateSettingId("wearingDetection")) and isToggleable())

    private fun waitForWearingDetection(expected: Boolean, waitForRow: Boolean = true) {
        composeRule.waitUntil(uiTimeoutMillis()) { wearingDetection() == expected }
        if (waitForRow) {
            composeRule.waitUntil(uiTimeoutMillis()) {
                wearingDetectionRow().fetchSemanticsNode().config[SemanticsProperties.ToggleableState] ==
                    if (expected) ToggleableState.On else ToggleableState.Off
            }
        }
        composeRule.waitForIdle()
    }

    private fun presetButton(@StringRes label: Int): SemanticsNodeInteraction {
        composeRule.onNode(hasScrollAction()).performScrollToNode(hasTestTag("sound_preset_$presetName"))
        val matcher = hasText(getString(label)) and hasAnyAncestor(hasTestTag("sound_preset_$presetName"))
        waitForNode(matcher)
        return composeRule.onNode(matcher)
    }

    private fun selectNavigation(@StringRes label: Int) {
        waitForNode(hasText(getString(label)) and isSelectable())
        composeRule.onNode(hasText(getString(label)) and isSelectable()).performClick()
        waitForNavigation(label)
    }

    private fun waitForNavigation(@StringRes label: Int) {
        waitForNode(hasText(getString(label)) and isSelectable())
        composeRule.onNode(hasText(getString(label)) and isSelectable()).assertIsSelected()
    }

    private fun waitForNode(matcher: SemanticsMatcher) {
        composeRule.waitUntil(uiTimeoutMillis()) { composeRule.onAllNodes(matcher).fetchSemanticsNodes().isNotEmpty() }
    }

    private fun pressBack() {
        composeRule.runOnIdle { composeRule.activity.onBackPressedDispatcher.onBackPressed() }
        composeRule.waitForIdle()
    }
}
