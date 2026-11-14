package com.oppzippy.openscq30.widget

import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.glance.text.FontWeight
import androidx.glance.text.TextStyle
import com.oppzippy.openscq30.features.preferences.EarbudsColor

/**
 * Text styles of the widget.
 *
 * Glance cannot load the bundled Onest font: font families in widgets are system family names only, so the widget
 * approximates the client's typography with the system typeface, the same sizes and weights as
 * android/app/src/main/java/com/oppzippy/openscq30/ui/theme/Type.kt. Colours are applied per use from GlanceTheme.
 */
internal object WidgetText {
    val headerName = TextStyle(fontWeight = FontWeight.Bold, fontSize = 15.sp)
    val headerDetail = TextStyle(fontWeight = FontWeight.Medium, fontSize = 11.sp)
    val badge = TextStyle(fontWeight = FontWeight.Medium, fontSize = 10.sp)
    val groupHeading = TextStyle(fontWeight = FontWeight.Bold, fontSize = 11.sp)
    val status = TextStyle(fontWeight = FontWeight.Bold, fontSize = 14.sp)
    val rowLabel = TextStyle(fontWeight = FontWeight.Medium, fontSize = 14.sp)
    val selectLabel = TextStyle(fontWeight = FontWeight.Medium, fontSize = 11.sp)
    val value = TextStyle(fontSize = 13.sp)
    val reason = TextStyle(fontSize = 12.sp)
    val pill = TextStyle(fontWeight = FontWeight.Medium, fontSize = 13.sp)
    val button = TextStyle(fontWeight = FontWeight.Medium, fontSize = 13.sp)
}

/** The widget's geometry, matching the rounded cards and spacing of the OpenSCL4 client. */
internal object WidgetDimens {
    val rootPadding = 6.dp
    val compactRootPadding = 4.dp
    val sectionGap = 4.dp

    val headerRadius = 24.dp
    val headerPaddingHorizontal = 8.dp
    val headerPaddingVertical = 4.dp
    val headerArtSize = 30.dp
    val compactHeaderArtSize = 24.dp
    val headerArtGap = 6.dp
    val headerLogoSize = 22.dp
    val disconnectSize = 32.dp
    val disconnectIconSize = 20.dp
    val headerDisconnectGap = 2.dp
    val badgeRadius = 8.dp
    val headerBadgeGap = 4.dp
    val badgePaddingHorizontal = 4.dp
    val badgePaddingVertical = 2.dp

    val shortStatusRowPaddingHorizontal = 4.dp
    val shortStatusRowPaddingVertical = 4.dp
    val shortStatusRowIconSize = 18.dp
    val shortStatusRowGap = 6.dp

    val groupIconSize = 14.dp
    val groupIconGap = 4.dp
    val groupHeadingPaddingStart = 4.dp
    val groupHeadingPaddingTop = 6.dp
    val groupHeadingPaddingBottom = 2.dp

    val rowRadius = 20.dp
    val rowPaddingHorizontal = 10.dp
    val rowPaddingVertical = 6.dp
    val compactRowPaddingVertical = 4.dp
    val rowGap = 4.dp
    val rowControlGap = 8.dp
    val rowValueGap = 4.dp
    val rowReasonGap = 2.dp
    val blockedReasonGap = 4.dp
    val hueSwatchSize = 16.dp

    val noticeRadius = 12.dp
    val noticePaddingHorizontal = 6.dp
    val noticePaddingVertical = 4.dp

    val selectLabelGap = 4.dp
    val selectPillGap = 6.dp

    /** Room one pill needs before the options become radio rows instead. */
    val selectPillMinimumWidth = 80.dp

    /** Root, row and spare padding the pills do not have available inside the available widget width. */
    val pillRowReservedWidth = 40.dp
    val pillRadius = 12.dp
    val pillOuterPaddingHorizontal = 2.dp
    val pillOuterPaddingVertical = 4.dp
    val pillCompactOuterPaddingVertical = 2.dp
    val pillPaddingHorizontal = 6.dp
    val pillPaddingVertical = 4.dp

    val buttonRadius = 14.dp
    val buttonPaddingHorizontal = 16.dp
    val buttonPaddingVertical = 8.dp

    val centeredContentPaddingHorizontal = 12.dp
    val messageActionGap = 10.dp
    val pairedDeviceRowPaddingVertical = 8.dp

    /** Below this height the header and the group headings are replaced by one slim status row. */
    val shortHeight = 96.dp
}

/**
 * How the widget draws its chrome: the layout the preference and the available height allow, and the theme and earbuds
 * colour the Glance theme cannot know about on its own.
 */
internal data class WidgetChrome(
    /** The user's compact-layout preference: shorter rows and no group icons; group headings stay. */
    val compactPreference: Boolean,
    /** The widget is too short for the header and the group headings; one slim status row replaces them. */
    val isShort: Boolean,
    val isDarkTheme: Boolean,
    val earbudsColor: EarbudsColor?,
) {
    /** Rows and paddings shrink for the compact preference and, further, for a short widget. */
    val compact: Boolean get() = compactPreference || isShort
}
