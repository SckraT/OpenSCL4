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

/** Light themes show the silver Liberty 4 Pro and dark themes the black one, so it stands out from the card. */
@DrawableRes
internal fun earbudsArtImage(scheme: ColorScheme): Int = if (scheme.surface.luminance() >= 0.5f) {
    R.drawable.sound_liberty_4_pro_silver
} else {
    R.drawable.sound_liberty_4_pro_black
}

/** Decorative Liberty 4 Pro earbuds for the device overview. */
@Composable
fun SoundEarbudsArt(modifier: Modifier = Modifier) {
    Image(
        painterResource(earbudsArtImage(MaterialTheme.colorScheme)),
        contentDescription = null,
        contentScale = ContentScale.Fit,
        modifier = modifier,
    )
}
