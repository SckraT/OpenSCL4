package com.oppzippy.openscq30.ui.sound

import androidx.annotation.DrawableRes
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.painterResource
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.features.preferences.EarbudsColor
import com.oppzippy.openscq30.features.preferences.preferredEarbudsColor

/** What kind of device a model is, for filtering the model list. */
enum class DeviceKind {
    Earbuds,
    OverEar,
    OpenEar,
    Speaker,
}

/** Unbranded illustrations drawn in one shared style. Visually similar models share one. */
enum class DeviceArt(@DrawableRes val silver: Int, @DrawableRes val black: Int, val kind: DeviceKind) {
    Headphones(R.drawable.sound_art_headphones_silver, R.drawable.sound_art_headphones_black, DeviceKind.OverEar),
    MotionPlus(R.drawable.sound_art_motion_plus_silver, R.drawable.sound_art_motion_plus_black, DeviceKind.Speaker),
    OpenClip(R.drawable.sound_art_open_clip_silver, R.drawable.sound_art_open_clip_black, DeviceKind.OpenEar),
    OpenHook(R.drawable.sound_art_open_hook_silver, R.drawable.sound_art_open_hook_black, DeviceKind.OpenEar),
    SportHook(R.drawable.sound_art_sport_hook_silver, R.drawable.sound_art_sport_hook_black, DeviceKind.Earbuds),
    WingedBuds(R.drawable.sound_art_buds_winged_silver, R.drawable.sound_art_buds_winged_black, DeviceKind.Earbuds),
    CompactBuds(R.drawable.sound_art_buds_compact_silver, R.drawable.sound_art_buds_compact_black, DeviceKind.Earbuds),
    ClassicStem(R.drawable.sound_art_stem_classic_silver, R.drawable.sound_art_stem_classic_black, DeviceKind.Earbuds),
    LibertyStem(R.drawable.sound_art_stem_premium_silver, R.drawable.sound_art_stem_premium_black, DeviceKind.Earbuds),
    OpenStem(R.drawable.sound_art_stem_open_silver, R.drawable.sound_art_stem_open_black, DeviceKind.Earbuds),
}

private val deviceArtByModel: Map<String, DeviceArt> = mapOf(
    DeviceArt.Headphones to listOf(
        "A3004", "A3005", "A3027", "A3028", "A3029", "A3030", "A3031", "A3033", "A3035", "A3040", "A3062",
    ),
    DeviceArt.MotionPlus to listOf("A3116"),
    DeviceArt.OpenClip to listOf("A3330", "A3388", "D1101"),
    DeviceArt.OpenHook to listOf("A3876"),
    DeviceArt.SportHook to listOf("A3968"),
    DeviceArt.WingedBuds to listOf("A3909", "A3926", "A3930", "A3931", "A3935", "A3952"),
    DeviceArt.CompactBuds to listOf("A3936", "A3948", "D1301"),
    DeviceArt.ClassicStem to listOf("A3933", "A3939", "A3944", "A3949", "A3959", "D1202", "D1202C"),
    DeviceArt.LibertyStem to listOf("A3947", "A3951", "A3953", "A3954", "A3955", "A3957"),
    DeviceArt.OpenStem to listOf("A3945"),
).flatMap { (art, codes) -> codes.map { "Soundcore$it" to art } }.toMap()

/** The illustration of a device model such as "SoundcoreA3954", or null for models without one. */
fun deviceArt(model: String): DeviceArt? = deviceArtByModel[model]

/**
 * The illustration in the color chosen in the settings, or, when following the theme, silver in light themes and black
 * in dark themes, so it stands out from the card.
 */
@DrawableRes
internal fun deviceArtImage(art: DeviceArt, scheme: ColorScheme, color: EarbudsColor? = null): Int =
    when (color ?: if (scheme.surface.luminance() >= 0.5f) EarbudsColor.Silver else EarbudsColor.Black) {
        EarbudsColor.Silver -> art.silver
        EarbudsColor.Black -> art.black
    }

/** Decorative illustration of the device for the device overview and the device list. */
@Composable
fun SoundDeviceArt(art: DeviceArt, modifier: Modifier = Modifier, color: EarbudsColor? = preferredEarbudsColor()) {
    Image(
        painterResource(deviceArtImage(art, MaterialTheme.colorScheme, color)),
        contentDescription = null,
        contentScale = ContentScale.Fit,
        modifier = modifier,
    )
}

/** The illustration of the model, or a generic headphones icon for models without one. */
@Composable
fun DeviceArtOrIcon(model: String, modifier: Modifier = Modifier, dimmed: Boolean = false) {
    val artModifier = modifier.alpha(if (dimmed) 0.6f else 1f)
    val art = deviceArt(model)
    if (art != null) {
        SoundDeviceArt(art, artModifier)
    } else {
        Box(artModifier, contentAlignment = Alignment.Center) {
            Icon(
                painterResource(R.drawable.headphones_24px),
                contentDescription = null,
                modifier = Modifier.fillMaxSize(0.55f),
                tint = MaterialTheme.colorScheme.onSecondaryContainer,
            )
        }
    }
}
