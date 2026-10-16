package com.oppzippy.openscq30.ui.theme

import androidx.compose.material3.Typography
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontVariation
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import com.oppzippy.openscq30.R

// Onest includes Cyrillic and is distributed under the SIL Open Font License.
// The complete license is bundled in assets/licenses/OFL-Onest.txt.
private val Onest = FontFamily(
    Font(
        R.font.onest,
        weight = FontWeight.Normal,
        variationSettings = FontVariation.Settings(FontVariation.weight(400)),
    ),
    Font(
        R.font.onest,
        weight = FontWeight.Medium,
        variationSettings = FontVariation.Settings(FontVariation.weight(500)),
    ),
    Font(
        R.font.onest,
        weight = FontWeight.SemiBold,
        variationSettings = FontVariation.Settings(FontVariation.weight(600)),
    ),
    Font(R.font.onest, weight = FontWeight.Bold, variationSettings = FontVariation.Settings(FontVariation.weight(700))),
)

private fun soundTextStyle(size: Int, lineHeight: Int, weight: FontWeight = FontWeight.Normal) = TextStyle(
    fontFamily = Onest,
    fontWeight = weight,
    fontSize = size.sp,
    lineHeight = lineHeight.sp,
    letterSpacing = 0.sp,
)

val Typography = Typography(
    displayLarge = soundTextStyle(57, 64, FontWeight.Medium),
    displayMedium = soundTextStyle(45, 52, FontWeight.Medium),
    displaySmall = soundTextStyle(36, 44, FontWeight.Medium),
    headlineLarge = soundTextStyle(32, 40, FontWeight.SemiBold),
    headlineMedium = soundTextStyle(28, 36, FontWeight.SemiBold),
    headlineSmall = soundTextStyle(24, 32, FontWeight.SemiBold),
    titleLarge = soundTextStyle(22, 28, FontWeight.SemiBold),
    titleMedium = soundTextStyle(16, 24, FontWeight.Medium),
    titleSmall = soundTextStyle(14, 20, FontWeight.Medium),
    bodyLarge = soundTextStyle(16, 24),
    bodyMedium = soundTextStyle(14, 21),
    bodySmall = soundTextStyle(12, 18),
    labelLarge = soundTextStyle(14, 20, FontWeight.Medium),
    labelMedium = soundTextStyle(12, 16, FontWeight.Medium),
    labelSmall = soundTextStyle(11, 16, FontWeight.Medium),
)
