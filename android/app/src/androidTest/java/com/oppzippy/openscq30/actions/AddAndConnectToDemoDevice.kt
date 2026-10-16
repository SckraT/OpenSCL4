package com.oppzippy.openscq30.actions

import androidx.activity.ComponentActivity
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasScrollAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.AndroidComposeTestRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import com.oppzippy.openscq30.MainActivity
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.lib.bindings.translateDeviceModel
import com.oppzippy.openscq30.uiTimeoutMillis
import kotlinx.coroutines.runBlocking
import org.junit.rules.TestRule

fun <Rule : TestRule, A : ComponentActivity> addAndConnectToDemoDevice(
    composeRule: AndroidComposeTestRule<Rule, A>,
    modelName: String,
) {
    val savedDemo = (composeRule.activity as? MainActivity)?.let { activity ->
        runBlocking { activity.session.pairedDevices() }
            .firstOrNull { it.isDemo && translateDeviceModel(it.model) == modelName }
    }
    if (savedDemo == null) {
        addDemoDevice(composeRule, modelName)
    }
    val deviceMatcher = hasText(modelName) and if (savedDemo != null) {
        hasText(savedDemo.macAddress)
    } else {
        hasText(composeRule.activity.getString(R.string.demo))
    }
    // Pairing returns to the list before the native paired-device refresh completes.
    awaitListItem(composeRule, deviceMatcher)
    composeRule.onNode(deviceMatcher).performClick()
}

fun <Rule : TestRule, A : ComponentActivity> addDemoDevice(
    composeRule: AndroidComposeTestRule<Rule, A>,
    modelName: String,
) {
    val addDescription = composeRule.activity.getString(R.string.add)
    awaitListItem(composeRule, hasContentDescription(addDescription))
    composeRule.onNodeWithContentDescription(addDescription).performClick()
    composeRule.onNodeWithTag("modelList").performScrollToNode(hasText(modelName))
    composeRule.onNodeWithText(modelName).performClick()
    composeRule.onNodeWithText(composeRule.activity.getString(R.string.demo_mode)).performClick()
    awaitListItem(composeRule, hasText(modelName))
    composeRule.onNodeWithText(modelName).performClick()
    // Keep this helper usable by pairing-error tests: successful pairing is awaited
    // only by addAndConnectToDemoDevice, never by the operation under test itself.
}

/** Wait for asynchronous discovery/list refresh, then reveal a lazily composed item. */
internal fun <Rule : TestRule, A : ComponentActivity> awaitListItem(
    composeRule: AndroidComposeTestRule<Rule, A>,
    matcher: SemanticsMatcher,
) {
    composeRule.waitUntil(uiTimeoutMillis()) {
        if (composeRule.onAllNodes(hasScrollAction()).fetchSemanticsNodes().size != 1) {
            false
        } else {
            try {
                composeRule.onNode(hasScrollAction()).performScrollToNode(matcher)
                true
            } catch (error: AssertionError) {
                // Only a missing native-list result is transient. Keep all other
                // scrolling, semantics and assertion failures visible to the test.
                if (error.message?.startsWith("No node found that matches ") == true &&
                    error.message?.endsWith(" in scrollable container") == true
                ) {
                    false
                } else {
                    throw error
                }
            }
        }
    }
}
