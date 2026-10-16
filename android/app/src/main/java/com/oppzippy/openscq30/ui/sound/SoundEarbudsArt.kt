package com.oppzippy.openscq30.ui.sound

import androidx.annotation.DrawableRes
import androidx.compose.foundation.Image
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.painterResource
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.features.preferences.EarbudsColor
import com.oppzippy.openscq30.features.preferences.preferredEarbudsColor

/**
 * The earbuds chosen in the settings, or, when following the theme, the silver Liberty 4 Pro in light themes and the
 * black one in dark themes, so it stands out from the card.
 */
@DrawableRes
internal fun earbudsArtImage(scheme: ColorScheme, color: EarbudsColor? = null): Int =
    when (color ?: if (scheme.surface.luminance() >= 0.5f) EarbudsColor.Silver else EarbudsColor.Black) {
        EarbudsColor.Silver -> R.drawable.sound_liberty_4_pro_silver
        EarbudsColor.Black -> R.drawable.sound_liberty_4_pro_black
    }

/** Decorative Liberty 4 Pro earbuds for the device overview. */
@Composable
fun SoundEarbudsArt(modifier: Modifier = Modifier, color: EarbudsColor? = preferredEarbudsColor()) {
    Image(
        painterResource(earbudsArtImage(MaterialTheme.colorScheme, color)),
        contentDescription = null,
        contentScale = ContentScale.Fit,
        modifier = modifier,
    )
}
