package dev.wochap.wobook.ui.components

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.wochap.wobook.data.FaviconCache
import dev.wochap.wobook.domain.Favicons
import dev.wochap.wobook.domain.UrlDisplay
import com.adamglin.PhosphorIcons
import com.adamglin.phosphoricons.Regular
import com.adamglin.phosphoricons.regular.AndroidLogo
import com.adamglin.phosphoricons.regular.AppleLogo
import com.adamglin.phosphoricons.regular.ArrowSquareOut
import com.adamglin.phosphoricons.regular.ArrowsClockwise
import com.adamglin.phosphoricons.regular.Check
import com.adamglin.phosphoricons.regular.CheckCircle
import com.adamglin.phosphoricons.regular.CircleNotch
import com.adamglin.phosphoricons.regular.CloudSlash
import com.adamglin.phosphoricons.regular.Copy
import com.adamglin.phosphoricons.regular.Desktop
import com.adamglin.phosphoricons.regular.DeviceMobile
import com.adamglin.phosphoricons.regular.DotsThreeVertical
import com.adamglin.phosphoricons.regular.GearSix
import com.adamglin.phosphoricons.regular.Info
import com.adamglin.phosphoricons.regular.LinuxLogo
import com.adamglin.phosphoricons.regular.MagnifyingGlass
import com.adamglin.phosphoricons.regular.WarningCircle
import com.adamglin.phosphoricons.regular.WindowsLogo
import com.adamglin.phosphoricons.regular.X
import dev.wochap.wobook.domain.Formatting
import dev.wochap.wobook.ffi.DeviceView
import dev.wochap.wobook.ffi.Reachability
import dev.wochap.wobook.ffi.SyncState
import dev.wochap.wobook.ffi.SyncStatus
import dev.wochap.wobook.ui.theme.Dimens
import dev.wochap.wobook.ui.theme.Pill
import dev.wochap.wobook.ui.theme.Wb
import dev.wochap.wobook.ui.theme.WobookType

// ---------------------------------------------------------------------------
// Layout

/** Single column clamped to 640 dp and centred (large-screen rule). */
@Composable
fun ContentColumn(modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) {
    Box(modifier.fillMaxSize(), contentAlignment = Alignment.TopCenter) {
        Column(Modifier.widthIn(max = Dimens.maxContent).fillMaxSize(), content = content)
    }
}

@Composable
fun Clamp(modifier: Modifier = Modifier, content: @Composable BoxScope.() -> Unit) {
    Box(modifier.fillMaxWidth(), contentAlignment = Alignment.TopCenter) {
        Box(Modifier.widthIn(max = Dimens.maxContent).fillMaxWidth(), content = content)
    }
}

/** Small top app bar: 56 dp, leading icon, title, trailing actions. */
@Composable
fun AppBar(
    title: String?,
    navigationIcon: ImageVector?,
    onNavigate: () -> Unit,
    navigationLabel: String = "Back",
    actions: @Composable () -> Unit = {},
) {
    Row(
        Modifier.fillMaxWidth().height(Dimens.appBar).padding(horizontal = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (navigationIcon != null) {
            IconButton(onClick = onNavigate) { Icon(navigationIcon, navigationLabel, Modifier.size(24.dp)) }
        } else {
            Spacer(Modifier.width(12.dp))
        }
        Text(
            title ?: "",
            style = MaterialTheme.typography.titleLarge,
            modifier = Modifier.weight(1f).padding(start = 4.dp),
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        actions()
    }
}

/** 2 dp indeterminate line (sync / connecting). */
@Composable
fun SyncLine(visible: Boolean, modifier: Modifier = Modifier) {
    Box(modifier.fillMaxWidth().height(2.dp)) {
        if (visible) {
            LinearProgressIndicator(
                modifier = Modifier.fillMaxSize().testTag("sync-line"),
                color = MaterialTheme.colorScheme.primary,
                trackColor = Color.Transparent,
                strokeCap = StrokeCap.Round,
            )
        }
    }
}

// ---------------------------------------------------------------------------
// Buttons

enum class ButtonTone { Primary, Secondary, Destructive, Success }

/** Outlined button: primary = accent outline, secondary = outline, destructive = error. */
@Composable
fun WbButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    tone: ButtonTone = ButtonTone.Primary,
    icon: ImageVector? = null,
    enabled: Boolean = true,
    height: Dp = 48.dp,
) {
    val scheme = MaterialTheme.colorScheme
    val color = when (tone) {
        ButtonTone.Primary -> scheme.primary
        ButtonTone.Secondary -> scheme.onSurface
        ButtonTone.Destructive -> scheme.error
        ButtonTone.Success -> Wb.colors.success
    }
    val border = when (tone) {
        ButtonTone.Secondary -> scheme.outline
        else -> color
    }
    OutlinedButton(
        onClick = onClick,
        enabled = enabled,
        modifier = modifier.heightIn(min = height),
        shape = MaterialTheme.shapes.small,
        border = BorderStroke(1.dp, if (enabled) border else scheme.outlineVariant),
        contentPadding = PaddingValues(horizontal = 16.dp),
        colors = androidx.compose.material3.ButtonDefaults.outlinedButtonColors(contentColor = color),
    ) {
        if (icon != null) {
            Icon(icon, null, Modifier.size(18.dp))
            Spacer(Modifier.width(8.dp))
        }
        Text(text, style = MaterialTheme.typography.labelLarge)
    }
}

@Composable
fun WbTextButton(text: String, onClick: () -> Unit, modifier: Modifier = Modifier, color: Color = MaterialTheme.colorScheme.primary) {
    TextButton(onClick = onClick, modifier = modifier, shape = MaterialTheme.shapes.small) {
        Text(text, style = MaterialTheme.typography.labelLarge, color = color)
    }
}

// ---------------------------------------------------------------------------
// Chips

enum class ChipState { Default, Selected }

/** Filter chip: default (surface) or selected (container + 1 dp accent + check). */
@Composable
fun TagChip(tag: String, state: ChipState, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val scheme = MaterialTheme.colorScheme
    val selected = state == ChipState.Selected
    Row(
        modifier
            .height(Dimens.chip)
            .clip(Pill)
            .background(if (selected) scheme.primaryContainer else scheme.surfaceContainer)
            .then(if (selected) Modifier.border(1.dp, scheme.primary, Pill) else Modifier)
            .clickable(onClick = onClick)
            .padding(horizontal = 12.dp)
            .testTag("chip-$tag"),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        if (selected) Icon(PhosphorIcons.Regular.Check, null, Modifier.size(14.dp), tint = scheme.primary)
        Text(tag, style = MaterialTheme.typography.labelLarge, color = if (selected) scheme.primary else scheme.onSurfaceVariant, maxLines = 1)
    }
}

/** Removable editor chip: 28 dp, trailing ×. */
@Composable
fun RemovableChip(tag: String, onRemove: () -> Unit, modifier: Modifier = Modifier) {
    val scheme = MaterialTheme.colorScheme
    Row(
        modifier.height(28.dp).clip(Pill).background(scheme.surfaceContainer).padding(start = 10.dp, end = 0.dp).testTag("editor-chip-$tag"),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(tag, style = WobookType.labelMd, color = scheme.onSurface, maxLines = 1)
        Box(
            Modifier.size(28.dp).clip(CircleShape).clickable(onClick = onRemove),
            contentAlignment = Alignment.Center,
        ) { Icon(PhosphorIcons.Regular.X, "Remove $tag", Modifier.size(14.dp), tint = scheme.onSurfaceVariant) }
    }
}

/** Outlined suggestion chip with the matched part highlighted. */
@Composable
fun SuggestionChip(tag: String, match: IntRange?, onClick: () -> Unit) {
    val scheme = MaterialTheme.colorScheme
    val text = buildAnnotatedString {
        if (match == null) {
            append(tag)
        } else {
            append(tag.substring(0, match.first))
            withStyle(SpanStyle(color = scheme.primary, fontWeight = FontWeight.SemiBold)) {
                append(tag.substring(match.first, match.last + 1))
            }
            append(tag.substring(match.last + 1))
        }
    }
    Box(
        Modifier.height(Dimens.chip).clip(Pill).border(1.dp, scheme.outline, Pill).clickable(onClick = onClick)
            .padding(horizontal = 12.dp).testTag("suggestion-$tag"),
        contentAlignment = Alignment.Center,
    ) {
        Text(text, style = MaterialTheme.typography.labelLarge, color = if (match != null) scheme.onSurface else scheme.onSurfaceVariant, maxLines = 1)
    }
}

/** Tiny 20 dp row chip (grows with font scale). */
@Composable
fun TinyChip(text: String, overflow: Boolean = false) {
    val scheme = MaterialTheme.colorScheme
    val shape = MaterialTheme.shapes.extraSmall
    Box(
        Modifier.defaultMinSize(minHeight = 20.dp)
            .then(if (overflow) Modifier.border(1.dp, scheme.outline, shape) else Modifier.background(scheme.surfaceContainer, shape))
            .padding(horizontal = if (overflow) 6.dp else 7.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(text, style = MaterialTheme.typography.labelMedium, color = if (overflow) Wb.colors.textFaint else scheme.onSurfaceVariant, maxLines = 1)
    }
}

/** One clipped line of tiny chips with a `+N` overflow chip. */
@Composable
fun TagLine(tags: List<String>, maxShown: Int = 3) {
    if (tags.isEmpty()) return
    Row(horizontalArrangement = Arrangement.spacedBy(4.dp), modifier = Modifier.clip(RoundedCornerShape(0.dp))) {
        val shown = tags.take(maxShown)
        shown.forEach { TinyChip(it) }
        if (tags.size > shown.size) TinyChip("+${tags.size - shown.size}", overflow = true)
    }
}

// ---------------------------------------------------------------------------
// Text

/** Characters at [indices] in accent, weight 600. */
fun highlight(text: String, indices: List<Int>, accent: Color): AnnotatedString = buildAnnotatedString {
    val marks = indices.toHashSet()
    // Indices are char offsets (Unicode scalar values); walk code points.
    var i = 0
    var cp = 0
    while (i < text.length) {
        val next = text.offsetByCodePoints(i, 1)
        val chunk = text.substring(i, next)
        if (cp in marks) withStyle(SpanStyle(color = accent, fontWeight = FontWeight.SemiBold)) { append(chunk) } else append(chunk)
        i = next
        cp++
    }
}

@Composable
fun HighlightedText(text: String, indices: List<Int>, style: TextStyle, color: Color, modifier: Modifier = Modifier) {
    Text(
        highlight(text, indices, MaterialTheme.colorScheme.primary),
        style = style,
        color = color,
        maxLines = 1,
        overflow = TextOverflow.Ellipsis,
        modifier = modifier,
    )
}

// ---------------------------------------------------------------------------
// Search field

@Composable
fun SearchField(
    value: String,
    onValueChange: (String) -> Unit,
    focusRequester: FocusRequester,
    onSettings: () -> Unit,
    syncing: Boolean,
    modifier: Modifier = Modifier,
) {
    val scheme = MaterialTheme.colorScheme
    var focused by remember { mutableStateOf(false) }
    val shape = MaterialTheme.shapes.medium
    Row(
        modifier.fillMaxWidth().height(Dimens.field).clip(shape).background(scheme.surfaceContainer)
            .then(if (focused) Modifier.border(1.dp, scheme.primary, shape) else Modifier)
            .padding(start = 16.dp, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Icon(PhosphorIcons.Regular.MagnifyingGlass, null, Modifier.size(22.dp), tint = scheme.onSurfaceVariant)
        Box(Modifier.weight(1f), contentAlignment = Alignment.CenterStart) {
            if (value.isEmpty()) Text("Search bookmarks", style = MaterialTheme.typography.bodyLarge, color = Wb.colors.textFaint)
            BasicTextField(
                value = value,
                onValueChange = onValueChange,
                singleLine = true,
                textStyle = MaterialTheme.typography.bodyLarge.copy(color = scheme.onSurface),
                cursorBrush = SolidColor(scheme.primary),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                keyboardActions = KeyboardActions(onSearch = {}),
                modifier = Modifier.fillMaxWidth().focusRequester(focusRequester)
                    .onFocusChanged { focused = it.isFocused }.testTag("search-field"),
            )
        }
        when {
            value.isNotEmpty() -> IconButton(onClick = { onValueChange("") }, modifier = Modifier.size(44.dp).testTag("search-clear")) {
                Icon(PhosphorIcons.Regular.X, "Clear", Modifier.size(20.dp), tint = scheme.onSurfaceVariant)
            }
            syncing -> IconButton(onClick = onSettings, modifier = Modifier.size(44.dp).testTag("open-settings")) {
                Icon(PhosphorIcons.Regular.ArrowsClockwise, "Syncing", Modifier.size(22.dp), tint = scheme.primary)
            }
            else -> IconButton(onClick = onSettings, modifier = Modifier.size(44.dp).testTag("open-settings")) {
                Icon(PhosphorIcons.Regular.GearSix, "Settings", Modifier.size(22.dp), tint = scheme.onSurfaceVariant)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Result row

@OptIn(ExperimentalFoundationApi::class)
@Composable
fun ResultRow(
    title: String,
    titleIndices: List<Int>,
    displayUrl: String,
    urlIndices: List<Int>,
    tags: List<String>,
    onClick: () -> Unit,
    onLongClick: () -> Unit,
    onCopy: () -> Unit,
    onOpen: () -> Unit,
    modifier: Modifier = Modifier,
    icon: @Composable () -> Unit = { LetterTile(UrlDisplay.host(displayUrl)) },
) {
    val scheme = MaterialTheme.colorScheme
    Row(
        modifier.fillMaxWidth().heightIn(min = Dimens.row)
            .combinedClickable(onClick = onClick, onLongClick = onLongClick)
            .padding(start = 16.dp, end = 4.dp, top = 10.dp, bottom = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        icon()
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            HighlightedText(title.ifBlank { displayUrl }, if (title.isBlank()) emptyList() else titleIndices, WobookType.rowTitle, scheme.onSurface)
            HighlightedText(displayUrl, urlIndices, WobookType.mono, scheme.onSurfaceVariant)
            TagLine(tags)
        }
        Row {
            IconButton(onClick = onCopy, modifier = Modifier.size(Dimens.tap).testTag("row-copy")) {
                Icon(PhosphorIcons.Regular.Copy, "Copy URL", Modifier.size(22.dp), tint = scheme.onSurfaceVariant)
            }
            IconButton(onClick = onOpen, modifier = Modifier.size(Dimens.tap).padding(start = 0.dp).testTag("row-open")) {
                Icon(PhosphorIcons.Regular.ArrowSquareOut, "Open in browser", Modifier.size(22.dp), tint = scheme.primary)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Favicon slot

/** 20 dp tile with the host's first letter on a colour derived from the host. */
@Composable
fun LetterTile(host: String, modifier: Modifier = Modifier) {
    val scheme = MaterialTheme.colorScheme
    val wb = Wb.colors
    val palette = listOf(scheme.primary, wb.tailscale, scheme.error, wb.success, wb.warning)
    Box(
        modifier.size(20.dp).background(palette[Favicons.colorIndex(host)], MaterialTheme.shapes.extraSmall).testTag("letter-tile"),
        contentAlignment = Alignment.Center,
    ) {
        Text(Favicons.letter(host), style = WobookType.labelMd.copy(fontSize = 11.sp, lineHeight = 12.sp), color = scheme.onPrimary)
    }
}

/** Site icon for a bookmark URL; letter tile while loading, without an icon, or with icons off. */
@Composable
fun FaviconSlot(url: String, cache: FaviconCache, loadIcons: Boolean) {
    val origin = remember(url) { Favicons.origin(url) }
    val host = remember(url, origin) { origin?.let(Favicons::host) ?: UrlDisplay.host(url) }
    val bitmap by produceState(initialValue = origin?.takeIf { loadIcons }?.let(cache::peek), origin, loadIcons) {
        value = if (loadIcons && origin != null) cache.icon(origin) else null
    }
    val image = bitmap
    if (image != null) {
        Image(image, null, Modifier.size(20.dp).clip(MaterialTheme.shapes.extraSmall).testTag("favicon"))
    } else {
        LetterTile(host)
    }
}

// ---------------------------------------------------------------------------
// Fetch note, shimmer

enum class FetchState { None, Fetching, Fetched, Failed, Offline }

@Composable
fun FetchStateNote(state: FetchState, text: String, onRetry: (() -> Unit)? = null) {
    if (state == FetchState.None) return
    val wb = Wb.colors
    val scheme = MaterialTheme.colorScheme
    val (icon, tint) = when (state) {
        FetchState.Fetching -> PhosphorIcons.Regular.CircleNotch to scheme.primary
        FetchState.Fetched -> PhosphorIcons.Regular.Check to wb.success
        FetchState.Failed -> PhosphorIcons.Regular.WarningCircle to wb.warning
        FetchState.Offline -> PhosphorIcons.Regular.CloudSlash to scheme.onSurfaceVariant
        FetchState.None -> return
    }
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp), modifier = Modifier.testTag("fetch-note")) {
        val spin = if (state == FetchState.Fetching) {
            val t = rememberInfiniteTransition(label = "spin")
            t.animateFloat(0f, 360f, infiniteRepeatable(tween(1000, easing = LinearEasing)), label = "deg").value
        } else 0f
        Icon(icon, null, Modifier.size(14.dp).then(Modifier.graphicsRotation(spin)), tint = tint)
        Text(text, style = MaterialTheme.typography.labelMedium, color = Wb.colors.textFaint, modifier = Modifier.weight(1f, fill = false))
        if (onRetry != null) {
            Text(
                "Retry",
                style = WobookType.labelMd,
                color = scheme.primary,
                modifier = Modifier.clip(MaterialTheme.shapes.extraSmall).clickable(onClick = onRetry).padding(horizontal = 6.dp, vertical = 4.dp),
            )
        }
    }
}

private fun Modifier.graphicsRotation(deg: Float): Modifier =
    if (deg == 0f) this else this.rotate(deg)

/** 14 dp shimmering bar replacing a field value while fetching. */
@Composable
fun ShimmerBar(fraction: Float = 0.7f) {
    val scheme = MaterialTheme.colorScheme
    val t = rememberInfiniteTransition(label = "shimmer")
    val x by t.animateFloat(-1f, 2f, infiniteRepeatable(tween(1400, easing = LinearEasing), RepeatMode.Restart), label = "x")
    val brush = Brush.linearGradient(
        colors = listOf(scheme.surfaceContainer, scheme.surfaceContainerHigh, scheme.surfaceContainer),
        start = androidx.compose.ui.geometry.Offset(x * 600f - 300f, 0f),
        end = androidx.compose.ui.geometry.Offset(x * 600f + 300f, 0f),
    )
    Box(Modifier.fillMaxWidth(fraction).height(14.dp).clip(MaterialTheme.shapes.extraSmall).background(brush).testTag("shimmer"))
}

// ---------------------------------------------------------------------------
// Fingerprint

@Composable
fun Fingerprint(groups: List<String>, modifier: Modifier = Modifier) {
    val scheme = MaterialTheme.colorScheme
    Column(
        modifier.fillMaxWidth().background(scheme.surfaceContainer, MaterialTheme.shapes.small).padding(16.dp).testTag("fingerprint"),
        verticalArrangement = Arrangement.spacedBy(6.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        groups.chunked(2).forEach { pair ->
            Row(horizontalArrangement = Arrangement.spacedBy(20.dp)) {
                pair.forEach { Text(it, style = WobookType.fingerprint, color = scheme.onSurface) }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Devices

@Composable
fun ReachabilityDot(reachability: Reachability) {
    val wb = Wb.colors
    val scheme = MaterialTheme.colorScheme
    val mod = Modifier.size(8.dp).testTag("dot-${reachability.name.lowercase()}")
    when (reachability) {
        Reachability.LAN -> Box(mod.background(wb.success, CircleShape))
        Reachability.TAILNET -> Box(mod.background(wb.tailscale, CircleShape))
        Reachability.UNREACHABLE -> Box(mod.border(1.dp, scheme.outline, CircleShape))
    }
}

fun platformIcon(platform: String): ImageVector = when (platform.lowercase()) {
    "linux" -> PhosphorIcons.Regular.LinuxLogo
    "android" -> PhosphorIcons.Regular.AndroidLogo
    "macos", "darwin" -> PhosphorIcons.Regular.AppleLogo
    "windows" -> PhosphorIcons.Regular.WindowsLogo
    else -> PhosphorIcons.Regular.Desktop
}

fun reachabilityLabel(r: Reachability): String = when (r) {
    Reachability.LAN -> "LAN"
    Reachability.TAILNET -> "Tailscale"
    Reachability.UNREACHABLE -> "Unreachable"
}

@Composable
fun ThisDeviceRow(name: String) {
    DeviceRowLayout(PhosphorIcons.Regular.DeviceMobile, name, "This device · Android", null, null, Modifier.testTag("device-this"))
}

@Composable
fun PeerRow(device: DeviceView, onMenu: () -> Unit, menu: @Composable () -> Unit) {
    val line = if (device.syncing) {
        "${reachabilityLabel(device.reachability)} · syncing…"
    } else {
        "${reachabilityLabel(device.reachability)} · synced ${Formatting.relative(device.lastSyncedMs)}"
    }
    DeviceRowLayout(platformIcon(device.platform), device.name, line, device.reachability, onMenu, Modifier.testTag("device-${device.name}"), menu)
}

@Composable
private fun DeviceRowLayout(
    icon: ImageVector,
    name: String,
    line: String,
    reachability: Reachability?,
    onMenu: (() -> Unit)?,
    modifier: Modifier,
    menu: @Composable () -> Unit = {},
) {
    val scheme = MaterialTheme.colorScheme
    Row(
        modifier.fillMaxWidth().heightIn(min = Dimens.row).padding(start = 16.dp, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Icon(icon, null, Modifier.size(24.dp), tint = scheme.onSurfaceVariant)
        Column(Modifier.weight(1f).padding(vertical = 10.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(name, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                if (reachability != null) ReachabilityDot(reachability)
                Text(line, style = MaterialTheme.typography.labelMedium, color = scheme.onSurfaceVariant, maxLines = 1)
            }
        }
        if (onMenu != null) {
            Box {
                IconButton(onClick = onMenu, modifier = Modifier.testTag("device-menu-$name")) {
                    Icon(PhosphorIcons.Regular.DotsThreeVertical, "More", Modifier.size(24.dp), tint = scheme.onSurfaceVariant)
                }
                menu()
            }
        }
    }
}

/** Icon + 12 sp label: Up to date / Syncing with <device> / No device reachable. */
@Composable
fun SyncStatusFooter(status: SyncStatus?, modifier: Modifier = Modifier) {
    val scheme = MaterialTheme.colorScheme
    val (icon, tint, text) = syncStatusParts(status)
    Row(modifier.padding(16.dp).testTag("sync-footer"), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Icon(icon, null, Modifier.size(16.dp), tint = tint ?: scheme.onSurfaceVariant)
        Text(text, style = MaterialTheme.typography.labelMedium, color = scheme.onSurfaceVariant)
    }
}

@Composable
fun syncStatusParts(status: SyncStatus?): Triple<ImageVector, Color?, String> {
    val wb = Wb.colors
    return when (val s = status?.state) {
        is SyncState.Syncing -> Triple(PhosphorIcons.Regular.ArrowsClockwise, MaterialTheme.colorScheme.primary, "Syncing with ${s.deviceName}")
        SyncState.UpToDate -> Triple(PhosphorIcons.Regular.CheckCircle, wb.success, "Up to date")
        SyncState.Disabled -> Triple(PhosphorIcons.Regular.CloudSlash, null, "Sync paused — changes saved locally")
        SyncState.NoPeerReachable, null -> Triple(PhosphorIcons.Regular.CloudSlash, null, "No device reachable — changes saved locally")
    }
}

// ---------------------------------------------------------------------------
// Empty state, info note, fields

@Composable
fun EmptyRow(icon: ImageVector, title: String, body: String) {
    val scheme = MaterialTheme.colorScheme
    Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        Icon(icon, null, Modifier.size(24.dp), tint = scheme.primary)
        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(title, style = MaterialTheme.typography.titleMedium)
            Text(body, style = MaterialTheme.typography.bodyMedium, color = scheme.onSurfaceVariant)
        }
    }
}

@Composable
fun InfoNote(text: String) {
    Row(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.Top) {
        Icon(PhosphorIcons.Regular.Info, null, Modifier.size(14.dp).padding(top = 1.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
        Text(text, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

/** Outlined labelled field (52 dp min), label inside the border like the design. */
@Composable
fun LabeledField(
    label: String,
    value: String,
    onValueChange: (String) -> Unit,
    modifier: Modifier = Modifier,
    placeholder: String = "",
    mono: Boolean = false,
    singleLine: Boolean = true,
    focusRequester: FocusRequester? = null,
    imeAction: ImeAction = ImeAction.Next,
    onDone: () -> Unit = {},
    tag: String = "",
    trailing: (@Composable () -> Unit)? = null,
    body: (@Composable () -> Unit)? = null,
) {
    val scheme = MaterialTheme.colorScheme
    var focused by remember { mutableStateOf(false) }
    val shape = MaterialTheme.shapes.small
    Column(
        modifier.fillMaxWidth().heightIn(min = Dimens.field).clip(shape)
            .border(1.dp, if (focused) scheme.primary else scheme.outlineVariant, shape)
            .padding(start = 14.dp, end = 14.dp, top = 6.dp, bottom = 8.dp),
        verticalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        Text(label, style = MaterialTheme.typography.labelMedium, color = if (focused) scheme.primary else scheme.onSurfaceVariant)
        if (body != null) {
            body()
        } else {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.weight(1f)) {
                    val style = (if (mono) WobookType.mono else MaterialTheme.typography.bodyLarge).copy(color = scheme.onSurface)
                    if (value.isEmpty() && placeholder.isNotEmpty()) Text(placeholder, style = style.copy(color = Wb.colors.textFaint))
                    BasicTextField(
                        value = value,
                        onValueChange = onValueChange,
                        singleLine = singleLine,
                        textStyle = style,
                        cursorBrush = SolidColor(scheme.primary),
                        keyboardOptions = KeyboardOptions(imeAction = imeAction),
                        keyboardActions = KeyboardActions(onDone = { onDone() }, onNext = null),
                        modifier = Modifier.fillMaxWidth()
                            .then(if (focusRequester != null) Modifier.focusRequester(focusRequester) else Modifier)
                            .onFocusChanged { focused = it.isFocused }
                            .then(if (tag.isNotEmpty()) Modifier.testTag(tag) else Modifier),
                    )
                }
                trailing?.invoke()
            }
        }
    }
}

@Composable
fun SectionLabel(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.labelLarge,
        color = MaterialTheme.colorScheme.primary,
        modifier = Modifier.padding(start = 16.dp, top = 20.dp, bottom = 4.dp),
    )
}

@Composable
fun Divider() {
    Box(Modifier.fillMaxWidth().height(1.dp).background(MaterialTheme.colorScheme.outlineVariant))
}

@Composable
fun VerticalFill() = Spacer(Modifier.fillMaxHeight())
