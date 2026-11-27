package com.oppzippy.openscq30.ui.theme

import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.glance.GlanceTheme
import androidx.glance.color.colorProviders
import androidx.glance.unit.ColorProvider

internal val lightColorScheme = lightColorScheme(
    primary = primaryLight,
    onPrimary = onPrimaryLight,
    primaryContainer = primaryContainerLight,
    onPrimaryContainer = onPrimaryContainerLight,
    secondary = secondaryLight,
    onSecondary = onSecondaryLight,
    secondaryContainer = secondaryContainerLight,
    onSecondaryContainer = onSecondaryContainerLight,
    tertiary = tertiaryLight,
    onTertiary = onTertiaryLight,
    tertiaryContainer = tertiaryContainerLight,
    onTertiaryContainer = onTertiaryContainerLight,
    error = errorLight,
    onError = onErrorLight,
    errorContainer = errorContainerLight,
    onErrorContainer = onErrorContainerLight,
    background = backgroundLight,
    onBackground = onBackgroundLight,
    surface = surfaceLight,
    onSurface = onSurfaceLight,
    surfaceVariant = surfaceVariantLight,
    onSurfaceVariant = onSurfaceVariantLight,
    outline = outlineLight,
    outlineVariant = outlineVariantLight,
    scrim = scrimLight,
    inverseSurface = inverseSurfaceLight,
    inverseOnSurface = inverseOnSurfaceLight,
    inversePrimary = inversePrimaryLight,
    surfaceDim = surfaceDimLight,
    surfaceBright = surfaceBrightLight,
    surfaceContainerLowest = surfaceContainerLowestLight,
    surfaceContainerLow = surfaceContainerLowLight,
    surfaceContainer = surfaceContainerLight,
    surfaceContainerHigh = surfaceContainerHighLight,
    surfaceContainerHighest = surfaceContainerHighestLight,
)

internal val darkColorScheme = darkColorScheme(
    primary = primaryDark,
    onPrimary = onPrimaryDark,
    primaryContainer = primaryContainerDark,
    onPrimaryContainer = onPrimaryContainerDark,
    secondary = secondaryDark,
    onSecondary = onSecondaryDark,
    secondaryContainer = secondaryContainerDark,
    onSecondaryContainer = onSecondaryContainerDark,
    tertiary = tertiaryDark,
    onTertiary = onTertiaryDark,
    tertiaryContainer = tertiaryContainerDark,
    onTertiaryContainer = onTertiaryContainerDark,
    error = errorDark,
    onError = onErrorDark,
    errorContainer = errorContainerDark,
    onErrorContainer = onErrorContainerDark,
    background = backgroundDark,
    onBackground = onBackgroundDark,
    surface = surfaceDark,
    onSurface = onSurfaceDark,
    surfaceVariant = surfaceVariantDark,
    onSurfaceVariant = onSurfaceVariantDark,
    outline = outlineDark,
    outlineVariant = outlineVariantDark,
    scrim = scrimDark,
    inverseSurface = inverseSurfaceDark,
    inverseOnSurface = inverseOnSurfaceDark,
    inversePrimary = inversePrimaryDark,
    surfaceDim = surfaceDimDark,
    surfaceBright = surfaceBrightDark,
    surfaceContainerLowest = surfaceContainerLowestDark,
    surfaceContainerLow = surfaceContainerLowDark,
    surfaceContainer = surfaceContainerDark,
    surfaceContainerHigh = surfaceContainerHighDark,
    surfaceContainerHighest = surfaceContainerHighestDark,
)

enum class ThemeType {
    // The names of the themes must not be changed, since they are used by Preferences for (de)serialization
    Light,
    Dark,
}

@Composable
fun ThemeType?.prefersDarkTheme(): Boolean = when (this) {
    null -> isSystemInDarkTheme()
    ThemeType.Light -> false
    ThemeType.Dark -> true
}

@Composable
fun OpenSCQ30Theme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    dynamicColor: Boolean = true,
    content: @Composable () -> Unit,
) {
    val colorScheme = when {
        dynamicColor && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S -> {
            val context = LocalContext.current
            if (darkTheme) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
        }

        darkTheme -> darkColorScheme

        else -> lightColorScheme
    }

    MaterialTheme(
        colorScheme = colorScheme,
        typography = Typography,
        shapes = Shapes(
            extraSmall = RoundedCornerShape(12.dp),
            small = RoundedCornerShape(16.dp),
            medium = RoundedCornerShape(24.dp),
            large = RoundedCornerShape(28.dp),
            extraLarge = RoundedCornerShape(32.dp),
        ),
        content = content,
    )
}

// Glance themes expose fewer roles than a Compose ColorScheme, and the home screen widget reads as a set of cards on
// the app background: the OpenSCL4 surface-container tokens are mapped onto the roles a widget can use, so the widget
// keeps the warm background, white cards and lavender pills of the client.
private val widgetLightColorProviders = colorProviders(
    primary = ColorProvider(primaryLight),
    onPrimary = ColorProvider(onPrimaryLight),
    primaryContainer = ColorProvider(primaryContainerLight),
    onPrimaryContainer = ColorProvider(onPrimaryContainerLight),
    secondary = ColorProvider(secondaryLight),
    onSecondary = ColorProvider(onSecondaryLight),
    secondaryContainer = ColorProvider(secondaryContainerLight),
    onSecondaryContainer = ColorProvider(onSecondaryContainerLight),
    tertiary = ColorProvider(tertiaryLight),
    onTertiary = ColorProvider(onTertiaryLight),
    tertiaryContainer = ColorProvider(tertiaryContainerLight),
    onTertiaryContainer = ColorProvider(onTertiaryContainerLight),
    error = ColorProvider(errorLight),
    onError = ColorProvider(onErrorLight),
    errorContainer = ColorProvider(errorContainerLight),
    onErrorContainer = ColorProvider(onErrorContainerLight),
    background = ColorProvider(backgroundLight),
    onBackground = ColorProvider(onBackgroundLight),
    surface = ColorProvider(surfaceContainerLowestLight),
    onSurface = ColorProvider(onSurfaceLight),
    surfaceVariant = ColorProvider(surfaceContainerHighLight),
    onSurfaceVariant = ColorProvider(onSurfaceVariantLight),
    outline = ColorProvider(outlineLight),
    inverseOnSurface = ColorProvider(inverseOnSurfaceLight),
    inverseSurface = ColorProvider(inverseSurfaceLight),
    inversePrimary = ColorProvider(inversePrimaryLight),
    widgetBackground = ColorProvider(backgroundLight),
)

private val widgetDarkColorProviders = colorProviders(
    primary = ColorProvider(primaryDark),
    onPrimary = ColorProvider(onPrimaryDark),
    primaryContainer = ColorProvider(primaryContainerDark),
    onPrimaryContainer = ColorProvider(onPrimaryContainerDark),
    secondary = ColorProvider(secondaryDark),
    onSecondary = ColorProvider(onSecondaryDark),
    secondaryContainer = ColorProvider(secondaryContainerDark),
    onSecondaryContainer = ColorProvider(onSecondaryContainerDark),
    tertiary = ColorProvider(tertiaryDark),
    onTertiary = ColorProvider(onTertiaryDark),
    tertiaryContainer = ColorProvider(tertiaryContainerDark),
    onTertiaryContainer = ColorProvider(onTertiaryContainerDark),
    error = ColorProvider(errorDark),
    onError = ColorProvider(onErrorDark),
    errorContainer = ColorProvider(errorContainerDark),
    onErrorContainer = ColorProvider(onErrorContainerDark),
    background = ColorProvider(backgroundDark),
    onBackground = ColorProvider(onBackgroundDark),
    surface = ColorProvider(surfaceContainerDark),
    onSurface = ColorProvider(onSurfaceDark),
    surfaceVariant = ColorProvider(surfaceContainerHighestDark),
    onSurfaceVariant = ColorProvider(onSurfaceVariantDark),
    outline = ColorProvider(outlineDark),
    inverseOnSurface = ColorProvider(inverseOnSurfaceDark),
    inverseSurface = ColorProvider(inverseSurfaceDark),
    inversePrimary = ColorProvider(inversePrimaryDark),
    widgetBackground = ColorProvider(backgroundDark),
)

@Composable
fun OpenSCQ30GlanceTheme(
    dynamicColor: Boolean = true,
    darkTheme: Boolean = LocalContext.current.systemInDarkTheme(),
    content: @Composable () -> Unit,
) {
    GlanceTheme(
        colors = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S && dynamicColor) {
            GlanceTheme.colors
        } else if (darkTheme) {
            widgetDarkColorProviders
        } else {
            widgetLightColorProviders
        },
    ) {
        content()
    }
}
