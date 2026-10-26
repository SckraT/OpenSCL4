package com.oppzippy.openscq30

import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assertIsNotSelected
import androidx.compose.ui.test.assertIsOff
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasScrollToIndexAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isOn
import androidx.compose.ui.test.isSelected
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performScrollToNode
import androidx.compose.ui.test.performTextInput
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.rule.GrantPermissionRule
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.UiObject2
import androidx.test.uiautomator.Until
import com.oppzippy.openscq30.actions.addAndConnectToDemoDevice
import com.oppzippy.openscq30.lib.bindings.translateDeviceModel
import com.oppzippy.openscq30.lib.bindings.translateSettingId
import dagger.hilt.android.testing.HiltAndroidTest
import java.util.regex.Pattern
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Before
import org.junit.Rule
import org.junit.Test

@HiltAndroidTest
class NotificationTest : OpenSCQ30RootTestBase() {
    @get:Rule(order = 0)
    val notificationPermissionRule: GrantPermissionRule =
        GrantPermissionRule.grant(android.Manifest.permission.POST_NOTIFICATIONS)

    private lateinit var uiDevice: UiDevice

    private val notificationTitle = By.text("Connected to ${translateDeviceModel("SoundcoreA3028")}")
    private val notification: UiObject2
        get() {
            return uiDevice.findObject(notificationTitle).parent.parent.parent!!
        }

    private fun connectDemo() {
        addAndConnectToDemoDevice(composeRule, translateDeviceModel("SoundcoreA3028"))
        composeRule.waitUntil(uiTimeoutMillis(10_000)) {
            composeRule.onAllNodesWithContentDescription(getString(R.string.sound_overview_device_actions))
                .fetchSemanticsNodes().isNotEmpty()
        }
    }

    private fun expandNotification() {
        notification.findObject(By.desc("Expand"))?.click()
        val disconnect = By.desc(getString(R.string.disconnect))
        notification.wait(Until.hasObject(disconnect), 5000)
    }

    @Before
    fun setUp() {
        uiDevice = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
    }

    @After
    fun tearDown() {
        uiDevice.pressHome()
    }

    @Test
    fun opensAppWhenNotificationIsClicked() {
        connectDemo()
        composeRule.waitForIdle()

        uiDevice.pressHome()
        uiDevice.openNotification()
        uiDevice.wait(Until.hasObject(notificationTitle), 5000)
        uiDevice.findObject(notificationTitle).click()
        val appPackage = InstrumentationRegistry.getInstrumentation().targetContext.packageName
        uiDevice.wait(Until.hasObject(By.pkg(appPackage)), 5000)
        assertEquals(appPackage, uiDevice.currentPackageName)
    }

    @Test
    fun disconnectsAndClosesNotificationWhenDisconnectIsClicked() {
        connectDemo()
        composeRule.waitForIdle()

        uiDevice.openNotification()
        uiDevice.wait(Until.hasObject(notificationTitle), 5000)
        expandNotification()

        val disconnect = By.desc(getString(R.string.disconnect))
        notification.findObject(disconnect).click()

        uiDevice.wait(Until.gone(notificationTitle), 5000)
        assertFalse(uiDevice.hasObject(notificationTitle))
        uiDevice.pressBack()

        composeRule.onNodeWithText(getString(R.string.sound_overview_disconnected)).assertExists()
        composeRule.onNodeWithText(getString(R.string.sound_overview_retry)).assertExists()
    }

    @Test
    fun quickPresetButtonsWork() {
        connectDemo()
        composeRule.onNodeWithText("Noise Canceling").performScrollTo().performClick().assertIsSelected()
        // Capture the current ambient sound mode and enable it in the preset editor.
        composeRule.onNode(hasText(getString(R.string.quick_presets)) and hasClickAction()).performClick()
        composeRule.onNodeWithText(getString(R.string.sound_preset_create)).performClick()
        composeRule.onNodeWithText(getString(R.string.name)).performTextInput("My Preset 1")
        composeRule.onNodeWithText(getString(R.string.create)).performClick()
        composeRule.onNodeWithText(getString(R.string.sound_preset_edit)).performClick()
        composeRule.onNodeWithText(translateSettingId("ambientSoundMode"))
            .performScrollTo().assertIsOff().performClick()
        // the switch follows the saved preset, which is read back after the library stores the change
        waitForNode(hasText(translateSettingId("ambientSoundMode")) and isOn())
        composeRule.onNodeWithContentDescription(getString(R.string.back)).performClick()
        // Assign that preset to the real notification's first quick-preset slot.
        composeRule.onNode(hasText(getString(R.string.settings)) and hasClickAction()).performClick()
        // the settings page is a lazy list: its entries further down are composed only once scrolled to
        val notificationControls = hasText(getString(R.string.sound_notification_controls))
        composeRule.onNode(hasScrollToIndexAction()).performScrollToNode(notificationControls)
        composeRule.onNode(notificationControls).performClick()
        composeRule.onNodeWithTag(getString(R.string.quick_preset_slot_x, 1) + " select")
            .performScrollTo().performClick()
        composeRule.onNodeWithText("My Preset 1").performClick()
        composeRule.onNodeWithContentDescription(getString(R.string.back)).performClick()
        composeRule.onNode(hasText(getString(R.string.sound_overview_device)) and hasClickAction()).performClick()
        composeRule.onNodeWithText("Normal").performScrollTo().performClick().assertIsSelected()
        composeRule.onNodeWithText("Noise Canceling").assertIsNotSelected()
        composeRule.waitForIdle()

        // Activate the quick preset via the notification
        uiDevice.openNotification()
        uiDevice.wait(Until.hasObject(notificationTitle), 5000)
        expandNotification()
        notification.findObject(By.text(Pattern.compile("My Preset 1", Pattern.CASE_INSENSITIVE))).click()
        uiDevice.pressBack()

        // the notification's service applies the preset in the background
        waitForNode(hasText("Noise Canceling") and isSelected())
        composeRule.onNodeWithText("Normal").assertIsNotSelected()
    }

    private fun waitForNode(matcher: SemanticsMatcher) {
        composeRule.waitUntil(uiTimeoutMillis()) { composeRule.onAllNodes(matcher).fetchSemanticsNodes().isNotEmpty() }
    }
}
