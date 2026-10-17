package com.oppzippy.openscq30

import android.content.ClipData
import android.content.ClipboardManager
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.state.ToggleableState
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertIsNotSelected
import androidx.compose.ui.test.assertIsOff
import androidx.compose.ui.test.assertIsOn
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.assertTextContains
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isDialog
import androidx.compose.ui.test.isSelectable
import androidx.compose.ui.test.isToggleable
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performSemanticsAction
import androidx.compose.ui.test.performTextClearance
import androidx.compose.ui.test.performTextInput
import androidx.core.content.getSystemService
import com.oppzippy.openscq30.actions.addAndConnectToDemoDevice
import com.oppzippy.openscq30.extensions.assertRangeValueApproxEquals
import com.oppzippy.openscq30.lib.bindings.translateCategoryId
import com.oppzippy.openscq30.lib.bindings.translateDeviceModel
import com.oppzippy.openscq30.lib.bindings.translateSettingId
import dagger.hilt.android.testing.HiltAndroidTest
import org.junit.Assert.assertTrue
import org.junit.Test

@HiltAndroidTest
class SettingTest : OpenSCQ30RootTestBase() {
    private fun connectDemo(model: String) {
        addAndConnectToDemoDevice(composeRule, translateDeviceModel(model))
        composeRule.waitUntil(uiTimeoutMillis(10_000)) {
            composeRule.onAllNodesWithContentDescription(getString(R.string.sound_overview_device_actions))
                .fetchSemanticsNodes().isNotEmpty()
        }
        // The connection can be ready while native capability reads are still running in the background.
        waitForNode(hasText(getString(R.string.equalizer)))
    }

    private fun waitForNode(matcher: SemanticsMatcher) {
        composeRule.waitUntil(uiTimeoutMillis()) { composeRule.onAllNodes(matcher).fetchSemanticsNodes().isNotEmpty() }
    }

    private fun waitForSelected(text: String) = waitForNode(
        hasText(text) and SemanticsMatcher.expectValue(SemanticsProperties.Selected, true),
    )

    private fun waitForToggle(text: String, checked: Boolean) = waitForNode(
        hasText(text) and SemanticsMatcher.expectValue(
            SemanticsProperties.ToggleableState,
            if (checked) ToggleableState.On else ToggleableState.Off,
        ),
    )

    private fun openEqualizer() {
        composeRule.onNodeWithText(getString(R.string.equalizer)).performScrollTo().performClick()
    }

    private fun openProfileTransfer() {
        composeRule.onNodeWithText(getString(R.string.sound_profile_transfer_title)).performScrollTo().performClick()
        waitForNode(hasText(getString(R.string.sound_profile_paste)))
    }

    private fun openAllSettingsCategory(id: String) {
        composeRule.onNodeWithText(getString(R.string.sound_overview_all_settings)).performScrollTo().performClick()
        waitForNode(hasText(translateCategoryId(id)) and hasClickAction())
        composeRule.onNode(hasText(translateCategoryId(id)) and hasClickAction()).performScrollTo().performClick()
    }

    private fun saveEqualizerProfile(name: String) {
        composeRule.onNodeWithText(getString(R.string.sound_profile_save)).performScrollTo().performClick()
        composeRule.onNodeWithText(getString(R.string.name)).performTextInput(name)
        composeRule.onNodeWithText(getString(R.string.sound_profile_save_short)).performClick()
        val replace = composeRule.onAllNodes(hasText(getString(R.string.sound_profile_replace)) and hasClickAction())
        if (replace.fetchSemanticsNodes().isNotEmpty()) replace[0].performClick()
        waitForNode(hasText(name))
        composeRule.onNodeWithText(name).performScrollTo().assertExists()
    }

    private fun deleteEqualizerProfile(name: String) {
        composeRule.onNodeWithContentDescription(getString(R.string.sound_profile_more_named, name))
            .performScrollTo().performClick()
        composeRule.onNodeWithText(getString(R.string.delete)).performClick()
        composeRule.onNode(hasText(getString(R.string.delete)) and hasAnyAncestor(isDialog())).performClick()
        composeRule.waitUntil(uiTimeoutMillis()) {
            composeRule.onAllNodes(hasText(name)).fetchSemanticsNodes().isEmpty()
        }
        composeRule.onNodeWithText(name).assertDoesNotExist()
    }

    @Test
    fun testWindNoiseSuppression() {
        connectDemo("SoundcoreA3959")
        composeRule.onNodeWithText(getString(R.string.sound_overview_customize)).performScrollTo().performClick()
        composeRule.onNodeWithText(translateSettingId("windNoiseSuppression"))
            .performScrollTo()
            .assertIsOff()
            .performClick()
        waitForToggle(translateSettingId("windNoiseSuppression"), true)
        composeRule.onNodeWithText(translateSettingId("windNoiseSuppression")).assertIsOn()
    }

    @Test
    fun testCustomNoiseCanceling() {
        connectDemo("SoundcoreA3951")
        composeRule.onNodeWithText(getString(R.string.sound_overview_customize)).performScrollTo().performClick()
        composeRule.onNodeWithText("Noise Canceling").performScrollTo().performClick()
        composeRule.onNodeWithTag(translateSettingId("customNoiseCanceling") + " slider")
            .performScrollTo()
            .assertRangeValueApproxEquals(0f)
            .performSemanticsAction(SemanticsActions.SetProgress) { it(10f) }
        composeRule.waitUntil(uiTimeoutMillis()) {
            composeRule.onNodeWithTag(translateSettingId("customNoiseCanceling") + " slider")
                .fetchSemanticsNode().config[SemanticsProperties.ProgressBarRangeInfo].current == 10f
        }
        composeRule.onNodeWithContentDescription(getString(R.string.back)).performClick()
        composeRule.onNodeWithText(getString(R.string.sound_overview_customize)).performScrollTo().performClick()
        composeRule.waitUntil(uiTimeoutMillis()) {
            composeRule.onNodeWithTag(translateSettingId("customNoiseCanceling") + " slider")
                .fetchSemanticsNode().config[SemanticsProperties.ProgressBarRangeInfo].current == 10f
        }
        composeRule.onNodeWithTag(translateSettingId("customNoiseCanceling") + " slider")
            .performScrollTo().assertRangeValueApproxEquals(10f)
    }

    @Test
    fun testNoiseCancelingMode() {
        connectDemo("SoundcoreA3951")
        composeRule.onNodeWithText(getString(R.string.sound_overview_customize)).performScrollTo().performClick()
        composeRule.onNodeWithText("Noise Canceling").performClick()
        waitForSelected("Noise Canceling")
        composeRule.onNodeWithText("Noise Canceling").assertIsSelected()
        composeRule.onNodeWithText("Normal").performClick()
        waitForSelected("Normal")
        composeRule.onNodeWithText("Normal").assertIsSelected()
        composeRule.onNodeWithText("Noise Canceling").assertIsNotSelected()
        // The selection comes back from the device and survives leaving the page.
        composeRule.onNodeWithContentDescription(getString(R.string.back)).performClick()
        composeRule.onNodeWithText("Normal").assertIsSelected()
    }

    @Test
    fun testPresetEqualizerProfile() {
        connectDemo("SoundcoreA3951")
        openEqualizer()
        composeRule.onNodeWithText("Acoustic").performScrollTo().performClick()
        waitForSelected("Acoustic")
        composeRule.onNodeWithText("Acoustic").assertIsSelected()
        composeRule.onNodeWithContentDescription(getString(R.string.back)).performClick()
        openEqualizer()
        composeRule.onNodeWithText("Acoustic").performScrollTo().assertIsSelected()
    }

    @Test
    fun testCustomEqualizerProfile() {
        connectDemo("SoundcoreA3951")
        openEqualizer()
        saveEqualizerProfile("Test Profile 2")
        composeRule.onNodeWithText("Test Profile 2").performClick()
        waitForNode(hasContentDescription(getString(R.string.in_effect)))
        composeRule.onNodeWithContentDescription(getString(R.string.in_effect)).assertExists()
        deleteEqualizerProfile("Test Profile 2")
    }

    @Test
    fun testExportEqualizerProfile() {
        connectDemo("SoundcoreA3951")
        openEqualizer()
        saveEqualizerProfile("Test Profile")
        openProfileTransfer()
        composeRule.onNode(hasText("Test Profile") and isToggleable()).performScrollTo().performClick()
        waitForToggle("Test Profile", true)
        composeRule.onNode(hasText("Test Profile") and isToggleable()).assertIsOn()
        composeRule.onNodeWithText(getString(R.string.sound_profile_copy)).performScrollTo().performClick()
        // read on the test thread: waitUntil checks its condition outside of the UI thread
        fun copied() =
            composeRule.activity.getSystemService<ClipboardManager>()?.primaryClip?.getItemAt(0)?.text?.toString()
                .orEmpty()
        composeRule.waitUntil(uiTimeoutMillis()) { copied().contains(""""name":"Test Profile"""") }
        assertTrue(copied(), copied().startsWith("[") && copied().endsWith("]"))
    }

    @Test
    fun testEqualizer() {
        connectDemo("SoundcoreA3951")
        openEqualizer()
        composeRule.onNodeWithText(getString(R.string.sound_profile_enter_values)).performScrollTo().performClick()
        val frequency = getString(R.string.hz, 100)
        composeRule.onNode(hasText(frequency) and hasSetTextAction()).performTextClearance()
        composeRule.onNode(hasText(frequency) and hasSetTextAction()).performTextInput("2")
        composeRule.onNodeWithText(getString(R.string.sound_profile_apply)).performScrollTo().performClick()
        val bandDescription = getString(R.string.sound_profile_band_accessibility, frequency, "2")
        waitForNode(hasContentDescription(bandDescription))
        composeRule.onNodeWithContentDescription(bandDescription).performScrollTo().assertExists()
        composeRule.onNodeWithContentDescription(bandDescription)
            .performSemanticsAction(SemanticsActions.SetProgress) { it(-120f) }
        waitForNode(hasContentDescription(getString(R.string.sound_profile_band_accessibility, frequency, "-12")))
        composeRule.onNodeWithContentDescription(
            getString(R.string.sound_profile_band_accessibility, frequency, "-12"),
        ).assertExists()
        // Reopening numeric entry proves that the backend received the slider value.
        composeRule.onNodeWithText(getString(R.string.sound_profile_enter_values)).performScrollTo().performClick()
        composeRule.onNode(hasText(frequency) and hasSetTextAction()).assertTextContains("-12")
        composeRule.onNode(hasText(frequency) and hasSetTextAction()).performTextClearance()
        composeRule.onNode(hasText(frequency) and hasSetTextAction()).performTextInput("1000")
        composeRule.onNodeWithText(getString(R.string.sound_profile_apply)).performScrollTo().assertIsNotEnabled()
    }

    @Test
    fun testImportString() {
        connectDemo("SoundcoreA3951")
        openEqualizer()
        openProfileTransfer()
        val json = """[{"name": "test profile", "volumeAdjustments": [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7]}]"""
        composeRule.runOnIdle {
            composeRule.activity.getSystemService<ClipboardManager>()
                ?.setPrimaryClip(ClipData.newPlainText("profiles", json))
        }
        composeRule.onNodeWithText(getString(R.string.sound_profile_paste)).performScrollTo().performClick()
        // a look at the file comes first, so nothing is imported or replaced by surprise
        waitForNode(hasText(getString(R.string.sound_profile_import_preview_title)))
        composeRule.onNodeWithText(getString(R.string.sound_profile_import_new)).assertExists()
        val importButton = composeRule.activity.resources.getQuantityString(R.plurals.sound_profile_import_button, 1, 1)
        composeRule.onNodeWithText(importButton).performClick()
        val imported = composeRule.activity.resources.getQuantityString(R.plurals.sound_profile_imported, 1, 1)
        waitForNode(hasText(imported))
        waitForNode(hasContentDescription(getString(R.string.sound_profile_more_named, "test profile")))
    }

    @Test
    fun testPastingSomethingElseExplainsWhatIsWrong() {
        connectDemo("SoundcoreA3951")
        openEqualizer()
        openProfileTransfer()
        composeRule.runOnIdle {
            composeRule.activity.getSystemService<ClipboardManager>()
                ?.setPrimaryClip(ClipData.newPlainText("text", "not profiles"))
        }
        composeRule.onNodeWithText(getString(R.string.sound_profile_paste)).performScrollTo().performClick()
        waitForNode(hasText(getString(R.string.sound_profile_not_profiles)))
    }

    @Test
    fun testA3116PresetEqualizerProfile() {
        connectDemo("SoundcoreA3116")
        // All settings open the same equalizer page as the device screen, for the speaker as well.
        openAllSettingsCategory("equalizer")
        composeRule.onNode(hasText("Bass Up") and isSelectable()).performScrollTo().performClick()
        waitForSelected("Bass Up")
        composeRule.onNodeWithContentDescription(getString(R.string.back)).performClick()
        composeRule.onNodeWithContentDescription(getString(R.string.back)).performClick()
        openAllSettingsCategory("equalizer")
        composeRule.onNode(hasText("Bass Up") and isSelectable()).performScrollTo().assertIsSelected()
    }

    @Test
    fun testA3116CustomEqualizerProfile() {
        connectDemo("SoundcoreA3116")
        openEqualizer()
        saveEqualizerProfile("Test Profile")
        composeRule.onNodeWithText("Test Profile").performClick()
        waitForNode(hasContentDescription(getString(R.string.in_effect)))
        composeRule.onNodeWithContentDescription(getString(R.string.in_effect)).assertExists()
        deleteEqualizerProfile("Test Profile")
    }
}
