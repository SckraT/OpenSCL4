package com.oppzippy.openscq30.ui.sound

import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.zIndex
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.features.preferences.DeviceUiPreferencesViewModel
import kotlin.math.roundToInt

/**
 * Which blocks the device overview shows and in what order. Only blocks the device has are listed; the others keep
 * their place in the saved layout.
 */
@Composable
fun OverviewLayoutEditor(
    deviceModel: String,
    available: Set<OverviewBlock>,
    preferences: DeviceUiPreferencesViewModel = hiltViewModel(),
) {
    val stored by remember(deviceModel) { preferences.overviewLayoutFlow(deviceModel) }.collectAsState()
    val layout = OverviewLayout.parse(stored)
    fun save(newLayout: OverviewLayout) = preferences.setOverviewLayout(deviceModel, newLayout.serialize())
    val shown = layout.blocks.filter { it.first in available }

    // moves a block among the listed ones, by its index in that list
    fun move(from: Int, to: Int) {
        val fromBlock = shown.getOrNull(from)?.first ?: return
        val toBlock = shown.getOrNull(to)?.first ?: return
        val fromIndex = layout.blocks.indexOfFirst { it.first == fromBlock }
        val toIndex = layout.blocks.indexOfFirst { it.first == toBlock }
        save(layout.moved(fromIndex, toIndex))
    }

    Column(
        Modifier.verticalScroll(rememberScrollState()).padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(stringResource(R.string.sound_overview_edit_headline), style = MaterialTheme.typography.headlineMedium)
        Text(
            stringResource(R.string.sound_overview_edit_description),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(
            stringResource(R.string.sound_overview_presets),
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.primary,
            modifier = Modifier.padding(top = 8.dp),
        )
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OverviewPreset.entries.forEach { preset ->
                FilterChip(
                    selected = layout.preset == preset,
                    onClick = { save(OverviewLayout.of(preset)) },
                    label = { Text(stringResource(preset.title)) },
                )
            }
        }
        Text(
            stringResource(R.string.sound_overview_blocks),
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.primary,
            modifier = Modifier.padding(top = 8.dp),
        )
        var dragged by remember { mutableStateOf<OverviewBlock?>(null) }
        var dragOffset by remember { mutableFloatStateOf(0f) }
        var rowHeight by remember { mutableIntStateOf(0) }
        // a drag outlives compositions, so it reads the latest list and layout through these
        val currentShown by rememberUpdatedState(shown)
        val currentMove by rememberUpdatedState<(Int, Int) -> Unit> { from, to -> move(from, to) }
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            shown.forEachIndexed { index, (block, visible) ->
                key(block) {
                    val isDragged = dragged == block
                    val moveUp = stringResource(R.string.sound_overview_move_up)
                    val moveDown = stringResource(R.string.sound_overview_move_down)
                    Surface(
                        shape = RoundedCornerShape(20.dp),
                        color = MaterialTheme.colorScheme.surfaceContainerLowest,
                        shadowElevation = if (isDragged) 6.dp else 0.dp,
                        modifier = Modifier.fillMaxWidth()
                            .zIndex(if (isDragged) 1f else 0f)
                            .graphicsLayer { translationY = if (isDragged) dragOffset else 0f }
                            .onGloballyPositioned { if (rowHeight == 0) rowHeight = it.size.height }
                            .semantics {
                                customActions = listOfNotNull(
                                    CustomAccessibilityAction(moveUp) {
                                        move(index, index - 1)
                                        true
                                    }
                                        .takeIf { index > 0 },
                                    CustomAccessibilityAction(moveDown) {
                                        move(index, index + 1)
                                        true
                                    }
                                        .takeIf { index < shown.lastIndex },
                                )
                            },
                    ) {
                        Row(
                            Modifier.padding(start = 4.dp, end = 16.dp, top = 6.dp, bottom = 6.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(12.dp),
                        ) {
                            Icon(
                                painterResource(R.drawable.drag_indicator_24px),
                                contentDescription = null,
                                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                                modifier = Modifier.padding(12.dp).size(24.dp).pointerInput(block) {
                                    detectDragGestures(
                                        onDragStart = { dragged = block },
                                        onDragEnd = {
                                            dragged = null
                                            dragOffset = 0f
                                        },
                                        onDragCancel = {
                                            dragged = null
                                            dragOffset = 0f
                                        },
                                    ) { change, amount ->
                                        change.consume()
                                        dragOffset += amount.y
                                        // one row plus the gap between rows
                                        val step = rowHeight + 8.dp.toPx()
                                        val rows = if (step > 0) (dragOffset / step).roundToInt() else 0
                                        if (rows != 0) {
                                            val from = currentShown.indexOfFirst { it.first == block }
                                            val to = (from + rows).coerceIn(0, currentShown.lastIndex)
                                            if (to != from) {
                                                currentMove(from, to)
                                                dragOffset -= (to - from) * step
                                            }
                                        }
                                    }
                                },
                            )
                            Icon(painterResource(block.icon), null, tint = MaterialTheme.colorScheme.primary)
                            Text(
                                stringResource(block.title),
                                style = MaterialTheme.typography.titleMedium,
                                modifier = Modifier.weight(1f),
                            )
                            Switch(checked = visible, onCheckedChange = { save(layout.withVisibility(block, it)) })
                        }
                    }
                }
            }
        }
    }
}
