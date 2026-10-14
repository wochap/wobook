package dev.wochap.wobook.ui.home

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import androidx.compose.ui.unit.dp
import com.adamglin.PhosphorIcons
import com.adamglin.phosphoricons.Regular
import com.adamglin.phosphoricons.regular.ArrowSquareOut
import com.adamglin.phosphoricons.regular.Check
import com.adamglin.phosphoricons.regular.CheckSquare
import com.adamglin.phosphoricons.regular.Copy
import com.adamglin.phosphoricons.regular.DotsThreeVertical
import com.adamglin.phosphoricons.regular.Minus
import com.adamglin.phosphoricons.regular.PencilSimple
import com.adamglin.phosphoricons.regular.PushPin
import com.adamglin.phosphoricons.regular.ShareNetwork
import com.adamglin.phosphoricons.regular.Square
import com.adamglin.phosphoricons.regular.Tag
import com.adamglin.phosphoricons.regular.Trash
import com.adamglin.phosphoricons.regular.X
import dev.wochap.wobook.data.AppRepository
import dev.wochap.wobook.ffi.UpdateRequest
import dev.wochap.wobook.ffi.parseTags
import dev.wochap.wobook.ui.components.ButtonTone
import dev.wochap.wobook.ui.components.TagEditor
import dev.wochap.wobook.ui.components.TagHelper
import dev.wochap.wobook.ui.components.TagSuggestions
import dev.wochap.wobook.ui.components.WbButton
import dev.wochap.wobook.ui.theme.Dimens

/** Replaces the search row while selecting. [single] is the row when exactly one is selected. */
@Composable
internal fun SelectionBar(
    count: Int,
    allSelected: Boolean,
    pinLabel: String,
    single: RowItem?,
    onClose: () -> Unit,
    onSelectAll: (Boolean) -> Unit,
    onPin: () -> Unit,
    onTags: () -> Unit,
    onDelete: () -> Unit,
    onOpen: (RowItem) -> Unit,
    onCopy: (RowItem) -> Unit,
    onShare: (RowItem) -> Unit,
    onEdit: (RowItem) -> Unit,
    modifier: Modifier = Modifier,
) {
    val scheme = MaterialTheme.colorScheme
    var menu by remember { mutableStateOf(false) }
    Row(
        modifier.fillMaxWidth().heightIn(min = Dimens.field).testTag("select-bar"),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        BarButton(PhosphorIcons.Regular.X, "Close", "select-close", onClick = onClose)
        Text(
            "$count selected",
            style = MaterialTheme.typography.titleMedium,
            modifier = Modifier.weight(1f).padding(start = 4.dp).testTag("select-count"),
        )
        BarButton(
            if (allSelected) PhosphorIcons.Regular.Square else PhosphorIcons.Regular.CheckSquare,
            if (allSelected) "Deselect all" else "Select all", "select-all",
        ) { onSelectAll(!allSelected) }
        BarButton(PhosphorIcons.Regular.PushPin, pinLabel, "select-pin", onClick = onPin)
        BarButton(PhosphorIcons.Regular.Tag, "Tags", "select-tags", onClick = onTags)
        BarButton(PhosphorIcons.Regular.Trash, "Delete", "select-delete", tint = scheme.error, onClick = onDelete)
        if (single != null) {
            Box {
                BarButton(PhosphorIcons.Regular.DotsThreeVertical, "More", "select-more") { menu = true }
                DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                    fun act(f: (RowItem) -> Unit) = { menu = false; f(single) }
                    MenuItem(PhosphorIcons.Regular.ArrowSquareOut, "Open in browser", "sheet-open", act(onOpen))
                    MenuItem(PhosphorIcons.Regular.Copy, "Copy URL", "sheet-copy", act(onCopy))
                    MenuItem(PhosphorIcons.Regular.ShareNetwork, "Share", "sheet-share", act(onShare))
                    MenuItem(PhosphorIcons.Regular.PencilSimple, "Edit", "sheet-edit", act(onEdit))
                }
            }
        }
    }
}

@Composable
private fun BarButton(
    icon: ImageVector,
    label: String,
    tag: String,
    tint: androidx.compose.ui.graphics.Color = MaterialTheme.colorScheme.onSurfaceVariant,
    onClick: () -> Unit,
) {
    IconButton(onClick = onClick, modifier = Modifier.size(Dimens.tap).testTag(tag)) {
        Icon(icon, label, Modifier.size(22.dp), tint = tint)
    }
}

@OptIn(androidx.compose.ui.ExperimentalComposeUiApi::class)
@Composable
private fun MenuItem(icon: ImageVector, label: String, tag: String, onClick: () -> Unit) {
    DropdownMenuItem(
        text = { Text(label) },
        onClick = onClick,
        leadingIcon = { Icon(icon, null, Modifier.size(22.dp)) },
        // Popup window: the root's resource-id flag does not reach it.
        modifier = Modifier.semantics { testTagsAsResourceId = true }.testTag(tag),
    )
}

internal enum class TagState { All, Some, None }

private class TagEntry(val tag: String, val count: Int, val initial: TagState) {
    var state by mutableStateOf(if (initial == TagState.None) TagState.All else initial)

    /** Some -> All -> None -> Some for a partial tag; All <-> None otherwise. */
    fun cycle() {
        state = if (initial == TagState.Some) {
            when (state) { TagState.Some -> TagState.All; TagState.All -> TagState.None; TagState.None -> TagState.Some }
        } else {
            if (state == TagState.All) TagState.None else TagState.All
        }
    }
}

/** Builds one add/remove delta per row; rows with nothing to change are skipped. */
internal fun bulkTagRequests(rows: List<RowItem>, states: Map<String, TagState>): List<UpdateRequest> =
    rows.mapNotNull { row ->
        val add = states.filter { it.value == TagState.All && it.key !in row.tags }.keys.toList()
        val remove = states.filter { it.value == TagState.None && it.key in row.tags }.keys.toList()
        if (add.isEmpty() && remove.isEmpty()) null else UpdateRequest(row.url, null, null, null, add, remove)
    }

@OptIn(ExperimentalMaterial3Api::class, androidx.compose.ui.ExperimentalComposeUiApi::class)
@Composable
internal fun BulkTagsSheet(
    repo: AppRepository,
    rows: List<RowItem>,
    onDismiss: () -> Unit,
    onApply: (List<UpdateRequest>) -> Unit,
) {
    val n = rows.size
    val entries = remember {
        val counts = linkedMapOf<String, Int>()
        rows.forEach { r -> r.tags.forEach { counts[it] = (counts[it] ?: 0) + 1 } }
        mutableStateListOf<TagEntry>().apply {
            counts.forEach { (tag, c) -> add(TagEntry(tag, c, if (c == n) TagState.All else TagState.Some)) }
        }
    }
    var pending by remember { mutableStateOf("") }
    var known by remember { mutableStateOf<List<Pair<String, Long>>>(emptyList()) }
    LaunchedEffect(Unit) {
        known = runCatching { repo.tags().map { it.tag to it.count } }.getOrDefault(emptyList())
    }
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val scheme = MaterialTheme.colorScheme

    fun addNew(tags: List<String>) {
        tags.forEach { t ->
            val existing = entries.firstOrNull { it.tag == t }
            if (existing == null) entries.add(TagEntry(t, 0, TagState.None)) else existing.state = TagState.All
        }
    }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = scheme.surfaceContainerLow,
    ) {
        Column(
            Modifier.fillMaxWidth().navigationBarsPadding().padding(horizontal = 20.dp).padding(bottom = 12.dp)
                .verticalScroll(rememberScrollState())
                // The sheet is its own window, so the root's flag does not reach it.
                .semantics { testTagsAsResourceId = true }.testTag("tags-sheet"),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text("Tags for $n bookmarks", style = MaterialTheme.typography.titleMedium)
            entries.forEach { e ->
                val (icon, label) = when (e.state) {
                    TagState.All -> PhosphorIcons.Regular.CheckSquare to "all"
                    TagState.Some -> PhosphorIcons.Regular.Minus to "${e.count} of $n"
                    TagState.None -> PhosphorIcons.Regular.Square to "none"
                }
                Row(
                    Modifier.fillMaxWidth().height(Dimens.tap).clickable { e.cycle() }
                        .semantics { contentDescription = "${e.tag}: ${e.state.name.lowercase()}" }
                        .testTag("tags-entry-${e.tag}"),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Icon(icon, null, Modifier.size(22.dp), tint = if (e.state == TagState.None) scheme.onSurfaceVariant else scheme.primary)
                    Text(e.tag, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f))
                    Text(
                        if (e.initial == TagState.None) "(new)" else label,
                        style = MaterialTheme.typography.labelMedium,
                        color = scheme.onSurfaceVariant,
                    )
                }
            }
            TagEditor(
                chips = emptyList(), onChipsChange = { addNew(it) },
                pending = pending, onPendingChange = { pending = it },
                normalize = { parseTags(it) },
                background = scheme.surfaceContainerLow,
            )
            TagSuggestions(pending, known, entries.map { it.tag }, onPick = { addNew(listOf(it)); pending = "" })
            TagHelper()
            Spacer(Modifier.height(4.dp))
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End)) {
                WbButton("Cancel", onDismiss, tone = ButtonTone.Secondary, height = 40.dp, modifier = Modifier.testTag("tags-cancel"))
                WbButton(
                    "Apply",
                    {
                        // A tag still pending in the input counts as committed.
                        val extra = parseTags(pending)
                        if (extra.isNotEmpty()) addNew(extra)
                        onApply(bulkTagRequests(rows, entries.associate { it.tag to it.state }))
                    },
                    icon = PhosphorIcons.Regular.Check, height = 40.dp, modifier = Modifier.testTag("tags-apply"),
                )
            }
        }
    }
}
