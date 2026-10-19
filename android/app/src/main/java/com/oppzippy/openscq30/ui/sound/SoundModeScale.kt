package com.oppzippy.openscq30.ui.sound

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.dp
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.lib.bindings.clientSoundModeScaleLimits
import com.oppzippy.openscq30.lib.bindings.clientSoundModeScaleSettings
import com.oppzippy.openscq30.lib.bindings.clientSoundModeScaleValue
import com.oppzippy.openscq30.lib.bindings.clientSupportsSoundModeScale
import com.oppzippy.openscq30.lib.wrapper.Setting
import com.oppzippy.openscq30.lib.wrapper.Value
import com.oppzippy.openscq30.lib.wrapper.toValue
import kotlin.math.roundToInt

private val scaleLimits by lazy { clientSoundModeScaleLimits() }
val SCALE_MIN get() = scaleLimits[0]
val SCALE_NORMAL get() = scaleLimits[1]
val SCALE_MAX get() = scaleLimits[2]

private const val AMBIENT = "ambientSoundMode"

internal fun supportsSoundModeScale(model: String, values: Map<String, Setting>): Boolean =
    clientSupportsSoundModeScale(model, values)

internal fun soundModeScaleValue(values: Map<String, Setting>): Int? = clientSoundModeScaleValue(values)

internal fun soundModeScaleSettings(position: Int): List<Pair<String, Value>> =
    clientSoundModeScaleSettings(position).map { it.setting to it.value }

/** The sound modes as one scale, as in the Soundcore app, with airplane mode beside it when the device has one. */
@Composable
internal fun SoundModeScale(values: Map<String, Setting>, setSettings: (List<Pair<String, Value>>) -> Unit) {
    val ambient = values[AMBIENT] as? Setting.SelectSetting ?: return
    val current = soundModeScaleValue(values)
    // follows the finger while dragging, and the device otherwise
    var dragged by remember { mutableStateOf<Float?>(null) }
    val position = dragged ?: (current ?: SCALE_NORMAL).toFloat()
    val shown = position.roundToInt()
    val description = when {
        current == null && dragged == null -> ambient.setting.localizedOption(ambient.value) ?: ambient.value
        shown < SCALE_NORMAL -> stringResource(R.string.sound_scale_transparency, SCALE_NORMAL - shown)
        shown > SCALE_NORMAL -> stringResource(R.string.sound_scale_noise_canceling, shown - SCALE_NORMAL)
        else -> ambient.setting.localizedOption("Normal") ?: "Normal"
    }
    Surface(shape = RoundedCornerShape(28.dp), color = MaterialTheme.colorScheme.surfaceContainer) {
        Column(Modifier.fillMaxWidth().padding(18.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                Text(description, style = MaterialTheme.typography.titleSmall)
                Text(shown.toString(), style = MaterialTheme.typography.titleSmall)
            }
            Slider(
                value = position,
                onValueChange = { dragged = it },
                onValueChangeFinished = {
                    dragged?.let { setSettings(soundModeScaleSettings(it.roundToInt())) }
                    dragged = null
                },
                valueRange = SCALE_MIN.toFloat()..SCALE_MAX.toFloat(),
                steps = SCALE_MAX - SCALE_MIN - 1,
                modifier = Modifier.semantics { stateDescription = description },
            )
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                listOf("Transparency", "Normal", "NoiseCanceling").forEach { option ->
                    Text(
                        ambient.setting.localizedOption(option) ?: option,
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
            if ("AirplaneMode" in ambient.setting.options) {
                FilterChip(
                    selected = ambient.value == "AirplaneMode",
                    onClick = { setSettings(listOf(AMBIENT to "AirplaneMode".toValue())) },
                    label = { Text(ambient.setting.localizedOption("AirplaneMode") ?: "AirplaneMode") },
                    leadingIcon = {
                        Icon(painterResource(soundModeIcon("AirplaneMode")), null, Modifier.size(18.dp))
                    },
                )
            }
        }
    }
}
