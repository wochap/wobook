package dev.wochap.wobook.ui.components

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.RoundRect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.google.zxing.BarcodeFormat
import com.google.zxing.EncodeHintType
import com.google.zxing.qrcode.QRCodeWriter
import com.google.zxing.qrcode.decoder.ErrorCorrectionLevel
import dev.wochap.wobook.ui.theme.Wb

/** Boolean module matrix for [text], no quiet zone. */
fun qrMatrix(text: String): Array<BooleanArray> {
    val bits = QRCodeWriter().encode(
        text, BarcodeFormat.QR_CODE, 0, 0,
        mapOf(EncodeHintType.MARGIN to 0, EncodeHintType.ERROR_CORRECTION to ErrorCorrectionLevel.M),
    )
    return Array(bits.height) { y -> BooleanArray(bits.width) { x -> bits.get(x, y) } }
}

/**
 * QR on a light tile (modules in crust, readable in dark theme), quiet zone
 * 10 dp, radius 8, optional 3 dp countdown ring ([progress] 1 → 0).
 */
@Composable
fun QrTile(text: String, size: Dp, progress: Float? = null, modifier: Modifier = Modifier) {
    val wb = Wb.colors
    val accent = MaterialTheme.colorScheme.primary
    val track = MaterialTheme.colorScheme.surfaceContainer
    val matrix = remember(text) { qrMatrix(text) }
    val ring = 3.dp
    Box(modifier.size(size + 32.dp), contentAlignment = Alignment.Center) {
        if (progress != null) {
            Canvas(Modifier.size(size + 32.dp).testTag("qr-ring")) {
                val stroke = ring.toPx()
                val inset = stroke / 2
                val rect = Size(this.size.width - stroke, this.size.height - stroke)
                drawRoundRect(track, Offset(inset, inset), rect, CornerRadius(16.dp.toPx()), style = Stroke(stroke))
                // Progress as an arc-like sweep around the rounded square.
                drawArc(
                    accent, -90f, 360f * progress.coerceIn(0f, 1f), false,
                    Offset(inset, inset), rect, style = Stroke(stroke, cap = StrokeCap.Round),
                )
            }
        }
        Box(
            Modifier.size(size).background(wb.qrTile, MaterialTheme.shapes.small).padding(10.dp).testTag("qr-tile"),
        ) {
            Canvas(Modifier.size(size - 20.dp)) {
                val n = matrix.size
                val cell = this.size.width / n
                matrix.forEachIndexed { y, row ->
                    row.forEachIndexed { x, on ->
                        if (on) drawRect(wb.qrModule, Offset(x * cell, y * cell), Size(cell + 0.5f, cell + 0.5f))
                    }
                }
            }
        }
    }
}

/** 260 dp framing square with 36 dp corner brackets, 3 dp accent. */
@Composable
fun ScanFrame(modifier: Modifier = Modifier, dim: Color = Color(0x6611111B)) {
    val accent = MaterialTheme.colorScheme.primary
    Canvas(modifier.size(260.dp).testTag("scan-frame")) {
        val r = 20.dp.toPx()
        val len = 36.dp.toPx()
        val w = 3.dp.toPx()
        val s = size.width
        val outside = Path().apply {
            addRect(Rect(-4000f, -4000f, s + 4000f, s + 4000f))
            addRoundRect(RoundRect(0f, 0f, s, s, CornerRadius(r)))
            fillType = PathFillType.EvenOdd
        }
        drawPath(outside, dim)
        val corner = Path().apply {
            moveTo(0f, len)
            lineTo(0f, r)
            arcTo(Rect(0f, 0f, 2 * r, 2 * r), 180f, 90f, false)
            lineTo(len, 0f)
        }
        repeat(4) { i -> rotate(90f * i) { drawPath(corner, accent, style = Stroke(w, cap = StrokeCap.Round)) } }
    }
}
