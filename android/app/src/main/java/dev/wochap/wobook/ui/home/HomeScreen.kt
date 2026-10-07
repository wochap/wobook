package dev.wochap.wobook.ui.home

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.isImeVisible
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.material3.rememberSwipeToDismissBoxState
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
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.adamglin.PhosphorIcons
import com.adamglin.phosphoricons.Regular
import com.adamglin.phosphoricons.regular.ArrowSquareOut
import com.adamglin.phosphoricons.regular.Copy
import com.adamglin.phosphoricons.regular.PencilSimple
import com.adamglin.phosphoricons.regular.Plus
import com.adamglin.phosphoricons.regular.QrCode
import com.adamglin.phosphoricons.regular.Scan
import com.adamglin.phosphoricons.regular.ShareNetwork
import com.adamglin.phosphoricons.regular.Trash
import dev.wochap.wobook.data.AppRepository
import dev.wochap.wobook.data.TapBehaviour
import dev.wochap.wobook.domain.Formatting
import dev.wochap.wobook.domain.UrlDisplay
import dev.wochap.wobook.ffi.SyncState
import dev.wochap.wobook.ui.LocalSnackbar
import dev.wochap.wobook.ui.HomeState
import dev.wochap.wobook.ui.components.ButtonTone
import dev.wochap.wobook.ui.components.ChipState
import dev.wochap.wobook.ui.components.EmptyRow
import dev.wochap.wobook.ui.components.ResultRow
import dev.wochap.wobook.ui.components.SearchField
import dev.wochap.wobook.ui.components.SyncLine
import dev.wochap.wobook.ui.components.TagChip
import dev.wochap.wobook.ui.components.WbButton
import dev.wochap.wobook.ui.copyUrl
import dev.wochap.wobook.ui.deleteWithUndo
import dev.wochap.wobook.ui.openUrl
import dev.wochap.wobook.ui.shareUrl
import dev.wochap.wobook.ui.theme.Dimens
import dev.wochap.wobook.ui.theme.Wb
import dev.wochap.wobook.ui.theme.WobookType
import kotlinx.coroutines.delay

/** One row's data, from a search hit or a plain list. */
data class RowItem(
    val url: String,
    val title: String,
    val titleIndices: List<Int>,
    val displayUrl: String,
    val urlIndices: List<Int>,
    val tags: List<String>,
)

private var focusedOnce = false

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun HomeScreen(
    repo: AppRepository,
    state: HomeState,
    tap: TapBehaviour,
    onDetail: (String) -> Unit,
    onAdd: (String) -> Unit,
    onEdit: (String) -> Unit,
    onSettings: () -> Unit,
    onScan: () -> Unit,
    onShowQr: () -> Unit,
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val snackbar = LocalSnackbar.current
    val revision by repo.revision.collectAsState()
    val sync by repo.syncStatus.collectAsState()
    val syncing = sync?.state is SyncState.Syncing
    val focus = remember { FocusRequester() }
    val focusManager = LocalFocusManager.current
    val imeVisible = WindowInsets.isImeVisible

    var total by remember { mutableStateOf<Long?>(null) }
    var tags by remember { mutableStateOf<List<String>>(emptyList()) }
    var rows by remember { mutableStateOf<List<RowItem>?>(null) }
    var sheet by remember { mutableStateOf<RowItem?>(null) }

    LaunchedEffect(revision) {
        runCatching {
            total = repo.librarySize()
            tags = repo.tags().map { it.tag }
        }
        state.selected.retainAll(tags.toSet())
    }
    val selected = state.selected.toList()
    LaunchedEffect(state.query, selected, revision) {
        delay(60)
        rows = runCatching {
            if (state.query.isBlank()) {
                repo.list(selected).sortedByDescending { it.updatedMs }.map {
                    RowItem(it.url, it.title, emptyList(), UrlDisplay.format(it.url), emptyList(), it.tags)
                }
            } else {
                repo.search(state.query, selected).map { h ->
                    RowItem(
                        h.bookmark.url, h.bookmark.title, h.titleIndices.map { it.toInt() },
                        h.displayUrl, h.urlIndices.map { it.toInt() }, h.bookmark.tags,
                    )
                }
            }
        }.getOrDefault(emptyList())
    }
    LaunchedEffect(Unit) {
        if (!focusedOnce) {
            focusedOnce = true
            runCatching { focus.requestFocus() }
        }
    }

    val filtered = state.query.isNotBlank() || selected.isNotEmpty()
    val empty = total == 0L
    val list = rows
    val frame = when {
        empty -> "home-empty"
        list != null && list.isEmpty() && filtered -> "home-none"
        filtered -> "home-results"
        else -> "home-idle"
    }

    fun tapRow(item: RowItem) = when (tap) {
        TapBehaviour.Detail -> onDetail(item.url)
        TapBehaviour.Open -> openUrl(context, item.url)
    }

    Box(Modifier.fillMaxSize().testTag("home"), contentAlignment = Alignment.TopCenter) {
        Column(
            Modifier.widthIn(max = Dimens.maxContent).fillMaxSize().statusBarsPadding().imePadding().testTag(frame),
        ) {
            SearchField(
                state.query, { state.query = it }, focus, onSettings, syncing,
                Modifier.padding(start = 16.dp, end = 16.dp, top = 8.dp),
            )
            SyncLine(syncing, Modifier.padding(horizontal = 16.dp, vertical = 5.dp).testTag(if (syncing) "home-sync" else "home-sync-off"))
            if (tags.isNotEmpty()) {
                // Selected chips first so they stay visible while the row scrolls.
                val ordered = selected + tags.filterNot { it in selected }
                LazyRow(
                    contentPadding = PaddingValues(horizontal = 16.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    modifier = Modifier.fillMaxWidth().testTag("chip-row"),
                ) {
                    items(ordered, key = { it }) { tag ->
                        TagChip(tag, if (tag in selected) ChipState.Selected else ChipState.Default, onClick = {
                            if (tag in state.selected) state.selected.remove(tag) else state.selected.add(tag)
                        })
                    }
                }
            }
            Spacer(Modifier.height(8.dp))
            Box(Modifier.weight(1f).fillMaxWidth()) {
                when {
                    empty -> EmptyLibrary(onScan, onShowQr)
                    list == null -> Unit
                    list.isEmpty() && filtered -> NoResults(state.query) { onAdd(if (UrlDisplay.looksLikeUrl(state.query)) state.query.trim() else "") }
                    else -> ResultList(
                        items = list,
                        header = if (!filtered) "Recent · ${Formatting.count(total ?: list.size.toLong())} bookmarks" else null,
                        onTap = ::tapRow,
                        onLong = { sheet = it; focusManager.clearFocus() },
                        onCopy = { copyUrl(context, it.url) },
                        onOpen = { openUrl(context, it.url) },
                    )
                }
            }
            if (filtered && list != null && list.isNotEmpty()) {
                val by = if (selected.isNotEmpty()) " · filtered by ${selected.joinToString(", ")}" else ""
                Text(
                    "${Formatting.count(list.size.toLong())} of ${Formatting.count(total ?: 0)}$by",
                    style = MaterialTheme.typography.labelMedium,
                    color = Wb.colors.textFaint,
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp).testTag("result-count"),
                )
            }
        }
        AnimatedVisibility(
            visible = !imeVisible,
            enter = fadeIn(), exit = fadeOut(),
            modifier = Modifier.align(Alignment.BottomEnd).navigationBarsPadding().padding(16.dp),
        ) {
            AddFab { onAdd("") }
        }
    }

    sheet?.let { item ->
        ActionSheet(
            item = item,
            onDismiss = { sheet = null },
            onOpen = { openUrl(context, item.url) },
            onCopy = { copyUrl(context, item.url) },
            onShare = { shareUrl(context, item.url, item.title) },
            onEdit = { onEdit(item.url) },
            onDelete = { deleteWithUndo(scope, repo, snackbar, item.url, item.title) },
        )
    }
}

@Composable
private fun AddFab(onClick: () -> Unit) {
    val scheme = MaterialTheme.colorScheme
    Row(
        Modifier.height(56.dp).background(scheme.background, MaterialTheme.shapes.medium)
            .border(1.dp, scheme.primary, MaterialTheme.shapes.medium)
            .clickable(onClick = onClick).padding(horizontal = 20.dp).testTag("add-fab"),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Icon(PhosphorIcons.Regular.Plus, null, Modifier.size(22.dp), tint = scheme.primary)
        Text("Add", style = MaterialTheme.typography.labelLarge, color = scheme.primary)
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ResultList(
    items: List<RowItem>,
    header: String?,
    onTap: (RowItem) -> Unit,
    onLong: (RowItem) -> Unit,
    onCopy: (RowItem) -> Unit,
    onOpen: (RowItem) -> Unit,
) {
    val listState = rememberLazyListState()
    LazyColumn(state = listState, modifier = Modifier.fillMaxSize().testTag("result-list"), contentPadding = PaddingValues(bottom = 96.dp)) {
        if (header != null) {
            item(key = "header") {
                Text(
                    header,
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                )
            }
        }
        items(items, key = { it.url }) { item ->
            val swipe = rememberSwipeToDismissBoxState(confirmValueChange = { value ->
                if (value == SwipeToDismissBoxValue.StartToEnd) onCopy(item)
                false
            })
            SwipeToDismissBox(
                state = swipe,
                enableDismissFromEndToStart = false,
                backgroundContent = {
                    Box(
                        Modifier.fillMaxSize().background(MaterialTheme.colorScheme.surfaceContainer).padding(start = 24.dp),
                        contentAlignment = Alignment.CenterStart,
                    ) { Icon(PhosphorIcons.Regular.Copy, "Copy URL", Modifier.size(22.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant) }
                },
            ) {
                ResultRow(
                    title = item.title,
                    titleIndices = item.titleIndices,
                    displayUrl = item.displayUrl,
                    urlIndices = item.urlIndices,
                    tags = item.tags,
                    onClick = { onTap(item) },
                    onLongClick = { onLong(item) },
                    onCopy = { onCopy(item) },
                    onOpen = { onOpen(item) },
                    modifier = Modifier.background(MaterialTheme.colorScheme.background).testTag("row-${item.displayUrl}"),
                )
            }
        }
    }
}

@Composable
private fun EmptyLibrary(onScan: () -> Unit, onShowQr: () -> Unit) {
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 24.dp), verticalArrangement = Arrangement.spacedBy(20.dp)) {
        Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Text("No bookmarks yet", style = MaterialTheme.typography.titleLarge)
            Text("Two ways to fill this up.", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        EmptyRow(
            PhosphorIcons.Regular.ShareNetwork, "Share a link from any app",
            "In Chrome, tap Share → wobook. Add tags, Save. You're back in the browser in a second.",
        )
        EmptyRow(
            PhosphorIcons.Regular.QrCode, "Pair with your desktop",
            "Your library syncs directly between devices on the same network or tailnet. No account, no cloud.",
        )
        Row(Modifier.padding(start = 40.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            WbButton("Scan QR", onScan, icon = PhosphorIcons.Regular.Scan, height = 40.dp, modifier = Modifier.testTag("empty-scan"))
            WbButton("Show my QR", onShowQr, tone = ButtonTone.Secondary, height = 40.dp, modifier = Modifier.testTag("empty-show"))
        }
    }
}

@Composable
private fun NoResults(query: String, onAdd: () -> Unit) {
    Column(Modifier.fillMaxWidth().padding(24.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text("Nothing matches “$query”", style = MaterialTheme.typography.titleMedium, maxLines = 2, overflow = TextOverflow.Ellipsis)
        Text(
            "Fuzzy search looks at titles, URLs, descriptions and tags. Try fewer letters.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(8.dp))
        WbButton("Add a bookmark", onAdd, icon = PhosphorIcons.Regular.Plus, height = 40.dp, modifier = Modifier.testTag("none-add"))
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ActionSheet(
    item: RowItem,
    onDismiss: () -> Unit,
    onOpen: () -> Unit,
    onCopy: () -> Unit,
    onShare: () -> Unit,
    onEdit: () -> Unit,
    onDelete: () -> Unit,
) {
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = MaterialTheme.colorScheme.surfaceContainerLow,
        shape = MaterialTheme.shapes.large.copy(bottomStart = androidx.compose.foundation.shape.CornerSize(0.dp), bottomEnd = androidx.compose.foundation.shape.CornerSize(0.dp)),
        modifier = Modifier.testTag("home-sheet"),
    ) {
        Column(Modifier.fillMaxWidth().navigationBarsPadding().padding(bottom = 8.dp)) {
            Column(Modifier.padding(horizontal = 20.dp, vertical = 4.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(item.title.ifBlank { item.displayUrl }, style = MaterialTheme.typography.titleMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(item.displayUrl, style = WobookType.mono, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
            Spacer(Modifier.height(8.dp))
            fun act(f: () -> Unit) = { onDismiss(); f() }
            SheetItem(PhosphorIcons.Regular.ArrowSquareOut, "Open in browser", "sheet-open", act(onOpen))
            SheetItem(PhosphorIcons.Regular.Copy, "Copy URL", "sheet-copy", act(onCopy))
            SheetItem(PhosphorIcons.Regular.ShareNetwork, "Share", "sheet-share", act(onShare))
            SheetItem(PhosphorIcons.Regular.PencilSimple, "Edit", "sheet-edit", act(onEdit))
            SheetItem(PhosphorIcons.Regular.Trash, "Delete", "sheet-delete", act(onDelete), destructive = true)
        }
    }
}

@Composable
private fun SheetItem(icon: ImageVector, label: String, tag: String, onClick: () -> Unit, destructive: Boolean = false) {
    val color = if (destructive) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface
    Row(
        Modifier.fillMaxWidth().height(Dimens.tap + 8.dp).clickable(onClick = onClick).padding(horizontal = 20.dp).testTag(tag),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Icon(icon, null, Modifier.size(24.dp), tint = if (destructive) color else MaterialTheme.colorScheme.onSurfaceVariant)
        Text(label, style = MaterialTheme.typography.bodyLarge, color = color)
    }
}
