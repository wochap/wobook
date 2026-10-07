package dev.wochap.wobook.ui.theme

import androidx.compose.material3.Typography
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import dev.wochap.wobook.R

val Inter = FontFamily(
    Font(R.font.inter, FontWeight.Normal),
    Font(R.font.inter, FontWeight.Medium),
    Font(R.font.inter, FontWeight.SemiBold),
)

val JetBrainsMono = FontFamily(
    Font(R.font.jetbrains_mono, FontWeight.Normal),
    Font(R.font.jetbrains_mono, FontWeight.Medium),
)

private fun inter(size: Int, line: Int, weight: FontWeight = FontWeight.Normal) =
    TextStyle(fontFamily = Inter, fontSize = size.sp, lineHeight = line.sp, fontWeight = weight)

/** `--wb-t-*` → Material roles. */
val WobookTypography = Typography(
    headlineMedium = inter(28, 34, FontWeight.Medium),
    titleLarge = inter(20, 26, FontWeight.Medium),
    titleMedium = inter(16, 22, FontWeight.Medium),
    bodyLarge = inter(16, 24),
    bodyMedium = inter(14, 20),
    bodySmall = inter(12, 16),
    labelLarge = inter(14, 20, FontWeight.Medium),
    labelMedium = inter(12, 16),
    labelSmall = inter(12, 16, FontWeight.Medium),
)

object WobookType {
    /** `--wb-t-mono` 13/18: url host + path. */
    val mono = TextStyle(fontFamily = JetBrainsMono, fontSize = 13.sp, lineHeight = 18.sp)
    /** Fingerprint 18/26 Medium, letterSpacing .04em. */
    val fingerprint = TextStyle(
        fontFamily = JetBrainsMono, fontSize = 18.sp, lineHeight = 26.sp,
        fontWeight = FontWeight.Medium, letterSpacing = 0.04.em,
    )
    /** `--wb-t-label-md` 12/16 Medium. */
    val labelMd = inter(12, 16, FontWeight.Medium)
    /** Result title: body-lg with 22 line height. */
    val rowTitle = inter(16, 22)
}
