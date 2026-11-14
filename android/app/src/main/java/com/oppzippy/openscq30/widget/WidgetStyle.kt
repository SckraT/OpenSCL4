package com.oppzippy.openscq30.widget

import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.glance.text.FontWeight
import androidx.glance.text.TextStyle

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

    val headerRadius = 22.dp
    val headerPaddingHorizontal = 8.dp
    val headerPaddingVertical = 4.dp
    val headerArtSize = 30.dp
    val compactHeaderArtSize = 24.dp
    val disconnectSize = 32.dp
    val disconnectIconSize = 20.dp
    val badgeRadius = 8.dp

    val groupIconSize = 14.dp
    val groupHeadingPaddingStart = 4.dp
    val groupHeadingPaddingTop = 6.dp
    val groupHeadingPaddingBottom = 2.dp

    val rowRadius = 18.dp
    val rowPaddingHorizontal = 10.dp
    val rowPaddingVertical = 6.dp
    val compactRowPaddingVertical = 4.dp
    val rowGap = 4.dp

    val pillRadius = 12.dp
    val pillPaddingHorizontal = 8.dp
    val pillPaddingVertical = 4.dp

    val noticeRadius = 12.dp
    val buttonRadius = 14.dp
    val buttonPaddingHorizontal = 16.dp
    val buttonPaddingVertical = 8.dp

    /** Below this height the header card and the group headings are dropped so the settings still fit. */
    val shortHeight = 96.dp
}
