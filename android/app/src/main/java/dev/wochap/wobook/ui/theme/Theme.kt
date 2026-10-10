package dev.wochap.wobook.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.ReadOnlyComposable

/** Mocha in dark mode (default), Latte in light mode; follows the system. */
@Composable
fun WobookTheme(dark: Boolean = isSystemInDarkTheme(), content: @Composable () -> Unit) {
    val palette = if (dark) Mocha else Latte
    val colorScheme = palette.colorScheme(dark)
    CompositionLocalProvider(LocalWobookColors provides palette.wobookColors(dark)) {
        MaterialTheme(
            colorScheme = colorScheme,
            typography = WobookTypography,
            shapes = WobookShapes,
        ) {
            // No root Surface/Scaffold sets this, so unstyled Text/Icon would default to black.
            CompositionLocalProvider(LocalContentColor provides colorScheme.onBackground, content = content)
        }
    }
}

object Wb {
    val colors: WobookColors
        @Composable @ReadOnlyComposable get() = LocalWobookColors.current
}
