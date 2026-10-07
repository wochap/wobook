package dev.wochap.wobook.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.ReadOnlyComposable

/** Mocha in dark mode (default), Latte in light mode; follows the system. */
@Composable
fun WobookTheme(dark: Boolean = isSystemInDarkTheme(), content: @Composable () -> Unit) {
    val palette = if (dark) Mocha else Latte
    CompositionLocalProvider(LocalWobookColors provides palette.wobookColors(dark)) {
        MaterialTheme(
            colorScheme = palette.colorScheme(dark),
            typography = WobookTypography,
            shapes = WobookShapes,
            content = content,
        )
    }
}

object Wb {
    val colors: WobookColors
        @Composable @ReadOnlyComposable get() = LocalWobookColors.current
}
