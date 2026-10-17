package com.oppzippy.openscq30

import androidx.compose.ui.test.assertTextContains
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import com.oppzippy.openscq30.actions.addAndConnectToDemoDevice
import com.oppzippy.openscq30.features.equalizer.storage.LegacyEqualizerProfile
import com.oppzippy.openscq30.features.equalizer.storage.LegacyEqualizerProfileDao
import com.oppzippy.openscq30.lib.bindings.translateDeviceModel
import dagger.hilt.android.testing.HiltAndroidTest
import javax.inject.Inject
import kotlinx.coroutines.runBlocking
import org.junit.Test

@HiltAndroidTest
class MigrateLegacyEqualizerProfilesTest : OpenSCQ30RootTestBase() {
    @Inject
    lateinit var legacyEqualizerProfileDao: LegacyEqualizerProfileDao

    @Test
    fun testMigrateLegacyProfile(): Unit = runBlocking {
        legacyEqualizerProfileDao.insert(
            LegacyEqualizerProfile(
                "Test Legacy Profile",
                10,
                -20,
                30,
                40,
                50,
                60,
                70,
                80,
            ),
        )
        addAndConnectToDemoDevice(composeRule, translateDeviceModel("SoundcoreA3028"))

        composeRule.waitUntil(uiTimeoutMillis(10_000)) {
            composeRule.onAllNodesWithContentDescription(getString(R.string.sound_overview_device_actions))
                .fetchSemanticsNodes().isNotEmpty()
        }
        composeRule.waitUntil(uiTimeoutMillis()) {
            composeRule.onAllNodes(hasText(getString(R.string.sound_overview_all_settings)))
                .fetchSemanticsNodes().isNotEmpty()
        }
        composeRule.onNodeWithText(getString(R.string.sound_overview_all_settings)).performScrollTo().performClick()
        composeRule.waitUntil(uiTimeoutMillis()) {
            composeRule.onAllNodes(hasText(getString(R.string.sound_overview_legacy)) and hasClickAction())
                .fetchSemanticsNodes().isNotEmpty()
        }
        composeRule.onNode(hasText(getString(R.string.sound_overview_legacy)) and hasClickAction())
            .performScrollTo().performClick()
        composeRule.onNodeWithText("Test Legacy Profile").assertExists()
        composeRule.onNodeWithText(getString(R.string.migrate)).performClick()
        composeRule.onNodeWithContentDescription(getString(R.string.back)).performClick()
        composeRule.onNodeWithContentDescription(getString(R.string.back)).performClick()
        composeRule.waitUntil(uiTimeoutMillis()) {
            composeRule.onAllNodes(hasText(getString(R.string.equalizer))).fetchSemanticsNodes().isNotEmpty()
        }
        composeRule.onNodeWithText(getString(R.string.equalizer)).performScrollTo().performClick()
        composeRule.waitUntil(uiTimeoutMillis()) {
            composeRule.onAllNodes(hasText("Test Legacy Profile")).fetchSemanticsNodes().isNotEmpty()
        }
        composeRule.onNodeWithText("Test Legacy Profile").performScrollTo().performClick()
        composeRule.waitUntil(uiTimeoutMillis()) {
            composeRule.onAllNodes(
                hasContentDescription(getString(R.string.in_effect)),
            ).fetchSemanticsNodes().isNotEmpty()
        }
        composeRule.onNodeWithText(getString(R.string.sound_profile_enter_values)).performScrollTo().performClick()
        composeRule.onNode(hasText(getString(R.string.hz, 100)) and hasSetTextAction()).assertTextContains("1")
        composeRule.onNode(hasText(getString(R.string.hz, 200)) and hasSetTextAction()).assertTextContains("-2")
    }
}
