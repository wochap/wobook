package dev.wochap.wobook.ui.theme

import androidx.compose.material3.ColorScheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.compositeOver
import androidx.compose.ui.unit.dp

/** Catppuccin palette subset used by `design/project/wobook-tokens.css`. */
@Immutable
data class Palette(
    val base: Color, val mantle: Color, val crust: Color,
    val surface0: Color, val surface1: Color, val surface2: Color,
    val overlay1: Color, val subtext0: Color, val text: Color,
    val blue: Color, val lavender: Color, val red: Color, val green: Color, val yellow: Color,
    val scrim: Color, val onAccent: Color,
)

val Mocha = Palette(
    base = Color(0xFF1E1E2E), mantle = Color(0xFF181825), crust = Color(0xFF11111B),
    surface0 = Color(0xFF313244), surface1 = Color(0xFF45475A), surface2 = Color(0xFF585B70),
    overlay1 = Color(0xFF7F849C), subtext0 = Color(0xFFA6ADC8), text = Color(0xFFCDD6F4),
    blue = Color(0xFF89B4FA), lavender = Color(0xFFB4BEFE), red = Color(0xFFF38BA8),
    green = Color(0xFFA6E3A1), yellow = Color(0xFFF9E2AF),
    scrim = Color(0x9911111B), onAccent = Color(0xFF11111B),
)

val Latte = Palette(
    base = Color(0xFFEFF1F5), mantle = Color(0xFFE6E9EF), crust = Color(0xFFDCE0E8),
    surface0 = Color(0xFFCCD0DA), surface1 = Color(0xFFBCC0CC), surface2 = Color(0xFFACB0BE),
    overlay1 = Color(0xFF8C8FA1), subtext0 = Color(0xFF6C6F85), text = Color(0xFF4C4F69),
    blue = Color(0xFF1E66F5), lavender = Color(0xFF7287FD), red = Color(0xFFD20F39),
    green = Color(0xFF40A02B), yellow = Color(0xFFDF8E1D),
    scrim = Color(0x664C4F69), onAccent = Color(0xFFEFF1F5),
)

/** Semantic extras outside Material's colorScheme (readme: Tokens → Compose). */
@Immutable
data class WobookColors(
    val success: Color,
    val tailscale: Color,
    val warning: Color,
    val textFaint: Color,
    /** QR modules (crust) and tile (text color, i.e. light in dark theme). */
    val qrModule: Color,
    val qrTile: Color,
    val isDark: Boolean,
)

fun Palette.wobookColors(dark: Boolean) = WobookColors(
    success = green, tailscale = lavender, warning = yellow, textFaint = overlay1,
    qrModule = if (dark) crust else text, qrTile = if (dark) text else Color.White, isDark = dark,
)

val LocalWobookColors = staticCompositionLocalOf { Mocha.wobookColors(true) }

fun Palette.colorScheme(dark: Boolean): ColorScheme {
    // --wb-accent-container: accent at 16% over the background.
    val accentContainer = blue.copy(alpha = 0.16f).compositeOver(base)
    val errorContainer = red.copy(alpha = 0.16f).compositeOver(base)
    val scheme = if (dark) darkColorScheme() else lightColorScheme()
    return scheme.copy(
        primary = blue, onPrimary = onAccent, primaryContainer = accentContainer, onPrimaryContainer = blue,
        inversePrimary = blue,
        secondary = blue, onSecondary = onAccent, secondaryContainer = accentContainer, onSecondaryContainer = blue,
        tertiary = lavender, onTertiary = onAccent, tertiaryContainer = accentContainer, onTertiaryContainer = lavender,
        background = base, onBackground = text, surface = base, onSurface = text,
        surfaceVariant = surface0, onSurfaceVariant = subtext0, surfaceTint = blue,
        inverseSurface = text, inverseOnSurface = base,
        error = red, onError = onAccent, errorContainer = errorContainer, onErrorContainer = red,
        outline = surface2, outlineVariant = surface1, scrim = scrim,
        surfaceBright = surface0, surfaceDim = base,
        surfaceContainer = surface0, surfaceContainerHigh = surface1, surfaceContainerHighest = surface1,
        surfaceContainerLow = mantle, surfaceContainerLowest = crust,
    )
}

object Dimens {
    val tap = 48.dp
    val row = 64.dp
    val chip = 32.dp
    val field = 52.dp
    val appBar = 56.dp
    val maxContent = 640.dp
    val s1 = 4.dp; val s2 = 8.dp; val s3 = 12.dp; val s4 = 16.dp; val s5 = 20.dp; val s6 = 24.dp; val s8 = 32.dp
}
