package dev.wochap.wobook.ui.detail

import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import com.adamglin.PhosphorIcons
import com.adamglin.phosphoricons.Regular
import com.adamglin.phosphoricons.regular.ArrowLeft
import com.adamglin.phosphoricons.regular.ArrowSquareOut
import com.adamglin.phosphoricons.regular.Copy
import com.adamglin.phosphoricons.regular.PencilSimple
import com.adamglin.phosphoricons.regular.ShareNetwork
import com.adamglin.phosphoricons.regular.Trash
import dev.wochap.wobook.data.AppRepository
import dev.wochap.wobook.domain.Formatting
import dev.wochap.wobook.ffi.Bookmark
import dev.wochap.wobook.ui.LocalSnackbar
import dev.wochap.wobook.ui.components.AppBar
import dev.wochap.wobook.ui.components.ChipState
import dev.wochap.wobook.ui.components.ContentColumn
import dev.wochap.wobook.ui.components.TagChip
import dev.wochap.wobook.ui.components.WbButton
import dev.wochap.wobook.ui.copyUrl
import dev.wochap.wobook.ui.deleteWithUndo
import dev.wochap.wobook.ui.openUrl
import dev.wochap.wobook.ui.shareUrl
import dev.wochap.wobook.ui.theme.Wb
import dev.wochap.wobook.ui.theme.WobookType

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun DetailScreen(repo: AppRepository, url: String, onBack: () -> Unit, onEdit: () -> Unit, onDeleted: () -> Unit) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val snackbar = LocalSnackbar.current
    val revision by repo.revision.collectAsState()
    var bookmark by remember { mutableStateOf<Bookmark?>(null) }
    LaunchedEffect(url, revision) { bookmark = runCatching { repo.get(url) }.getOrNull() }
    val b = bookmark
    val scheme = MaterialTheme.colorScheme

    ContentColumn(Modifier.statusBarsPadding().navigationBarsPadding().testTag("detail")) {
        AppBar(null, PhosphorIcons.Regular.ArrowLeft, onBack) {
            IconButton(onClick = onEdit, modifier = Modifier.testTag("detail-edit")) {
                Icon(PhosphorIcons.Regular.PencilSimple, "Edit", Modifier.size(24.dp))
            }
            IconButton(
                onClick = {
                    if (b != null) {
                        deleteWithUndo(scope, repo, snackbar, b.url, b.title)
                        onDeleted()
                    }
                },
                modifier = Modifier.testTag("detail-delete"),
            ) { Icon(PhosphorIcons.Regular.Trash, "Delete", Modifier.size(24.dp)) }
        }
        if (b == null) return@ContentColumn
        Column(
            Modifier.weight(1f).fillMaxWidth().verticalScroll(rememberScrollState()).padding(horizontal = 20.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Text(b.title.ifBlank { b.url }, style = MaterialTheme.typography.titleLarge, modifier = Modifier.testTag("detail-title"))
            SelectionContainer {
                Text(b.url, style = WobookType.mono, color = scheme.primary, modifier = Modifier.testTag("detail-url"))
            }
            if (b.description.isNotBlank()) {
                Text(b.description, style = MaterialTheme.typography.bodyMedium, color = scheme.onSurfaceVariant)
            }
            if (b.tags.isNotEmpty()) {
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    b.tags.forEach { TagChip(it, ChipState.Default, onClick = {}) }
                }
            }
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text("Saved ${Formatting.dateTime(b.createdMs)}", style = MaterialTheme.typography.labelMedium, color = Wb.colors.textFaint)
                Text(
                    "Last changed ${Formatting.relative(b.updatedMs)}",
                    style = MaterialTheme.typography.labelMedium,
                    color = Wb.colors.textFaint,
                    modifier = Modifier.testTag("detail-changed"),
                )
            }
        }
        Row(
            Modifier.fillMaxWidth().padding(16.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            WbButton("Open", { openUrl(context, b.url) }, Modifier.weight(1f).testTag("detail-open"), icon = PhosphorIcons.Regular.ArrowSquareOut)
            SquareIcon(PhosphorIcons.Regular.Copy, "Copy URL", "detail-copy") { copyUrl(context, b.url) }
            SquareIcon(PhosphorIcons.Regular.ShareNetwork, "Share", "detail-share") { shareUrl(context, b.url, b.title) }
        }
    }
}

@Composable
private fun SquareIcon(icon: androidx.compose.ui.graphics.vector.ImageVector, label: String, tag: String, onClick: () -> Unit) {
    val scheme = MaterialTheme.colorScheme
    Box(
        Modifier.size(48.dp).border(1.dp, scheme.outline, MaterialTheme.shapes.small).background(scheme.background, MaterialTheme.shapes.small),
        contentAlignment = Alignment.Center,
    ) {
        IconButton(onClick = onClick, modifier = Modifier.size(48.dp).testTag(tag)) {
            Icon(icon, label, Modifier.size(22.dp), tint = scheme.onSurface)
        }
    }
}
