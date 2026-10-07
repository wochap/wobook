package dev.wochap.wobook.ui.settings

import android.content.Context
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
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
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import com.adamglin.PhosphorIcons
import com.adamglin.phosphoricons.Regular
import com.adamglin.phosphoricons.regular.ArrowLeft
import com.adamglin.phosphoricons.regular.ArrowsClockwise
import com.adamglin.phosphoricons.regular.CaretRight
import com.adamglin.phosphoricons.regular.DownloadSimple
import com.adamglin.phosphoricons.regular.PencilSimple
import com.adamglin.phosphoricons.regular.UploadSimple
import dev.wochap.wobook.BuildConfig
import dev.wochap.wobook.data.AppRepository
import dev.wochap.wobook.data.Settings
import dev.wochap.wobook.data.SettingsState
import dev.wochap.wobook.data.TapBehaviour
import dev.wochap.wobook.data.userMessage
import dev.wochap.wobook.domain.Formatting
import dev.wochap.wobook.ffi.InterchangeFormat
import dev.wochap.wobook.ffi.SyncState
import dev.wochap.wobook.ui.LocalSnackbar
import dev.wochap.wobook.ui.components.AppBar
import dev.wochap.wobook.ui.components.ContentColumn
import dev.wochap.wobook.ui.components.LabeledField
import dev.wochap.wobook.ui.components.SectionLabel
import dev.wochap.wobook.ui.components.WbTextButton
import dev.wochap.wobook.ui.theme.Wb
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    repo: AppRepository,
    settings: Settings,
    state: SettingsState,
    onBack: () -> Unit,
    onDevices: () -> Unit,
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val snackbar = LocalSnackbar.current
    val revision by repo.revision.collectAsState()
    val devices by repo.devices.collectAsState()
    val sync by repo.syncStatus.collectAsState()
    var deviceName by remember { mutableStateOf("") }
    var size by remember { mutableStateOf(0L) }
    var bytes by remember { mutableStateOf(0L) }
    var renaming by remember { mutableStateOf(false) }
    var importMenu by remember { mutableStateOf(false) }
    var exportMenu by remember { mutableStateOf(false) }
    var importFormat by remember { mutableStateOf(InterchangeFormat.JSONL) }
    var exportFormat by remember { mutableStateOf(InterchangeFormat.JSONL) }
    var licenses by remember { mutableStateOf(false) }

    LaunchedEffect(revision) {
        deviceName = runCatching { repo.thisDevice().name }.getOrDefault("")
        size = runCatching { repo.librarySize() }.getOrDefault(0)
        bytes = withContext(Dispatchers.IO) { File(context.filesDir, "wobook").walkTopDown().filter { it.isFile }.sumOf { it.length() } }
    }

    val importer = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri != null) scope.launch {
            val message = runCatching {
                val file = copyToCache(context, uri, "import")
                val report = repo.import(importFormat, file.absolutePath)
                file.delete()
                "Imported ${report.added} new, ${report.merged} merged, ${report.skipped} unchanged" +
                    if (report.errors.isNotEmpty()) " · ${report.errors.size} errors" else ""
            }.getOrElse { it.userMessage() }
            snackbar.showSnackbar(message)
        }
    }
    val exporter = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("*/*")) { uri ->
        if (uri != null) scope.launch {
            val message = runCatching {
                val tmp = File(context.cacheDir, "export.tmp")
                val count = repo.export(exportFormat, tmp.absolutePath)
                withContext(Dispatchers.IO) {
                    context.contentResolver.openOutputStream(uri, "wt")!!.use { out -> tmp.inputStream().use { it.copyTo(out) } }
                    tmp.delete()
                }
                "Exported $count bookmarks"
            }.getOrElse { it.userMessage() }
            snackbar.showSnackbar(message)
        }
    }

    val lastSync = sync?.lastSyncMs
    val syncedWith = devices.maxByOrNull { it.lastSyncedMs ?: 0 }?.takeIf { it.lastSyncedMs != null }?.name
    val stateText = when (val s = sync?.state) {
        SyncState.UpToDate -> "up to date"
        is SyncState.Syncing -> "syncing with ${s.deviceName}"
        else -> "no device reachable"
    }

    ContentColumn(Modifier.statusBarsPadding().navigationBarsPadding().testTag("settings")) {
        AppBar("Settings", PhosphorIcons.Regular.ArrowLeft, onBack)
        Column(Modifier.weight(1f).fillMaxWidth().verticalScroll(rememberScrollState()).padding(bottom = 24.dp)) {
            SectionLabel("This device")
            SettingRow("Device name", "$deviceName · shown to peers when pairing", "settings-name", trailing = {
                Icon(PhosphorIcons.Regular.PencilSimple, null, Modifier.size(22.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
            }) { renaming = true }
            SettingRow("Devices", "${devices.size} paired · $stateText", "settings-devices", trailing = {
                Icon(PhosphorIcons.Regular.CaretRight, null, Modifier.size(20.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
            }, onClick = onDevices)

            SectionLabel("Sync")
            SettingRow("Background sync", "Every 15 min on Wi-Fi", "settings-background", trailing = {
                WbSwitch(state.backgroundSync) { on -> scope.launch { settings.setBackgroundSync(on) } }
            }) { scope.launch { settings.setBackgroundSync(!state.backgroundSync) } }
            SettingRow(
                "Sync now",
                if (lastSync == null) "Never synced" else "Last sync ${Formatting.relative(lastSync)}" + (syncedWith?.let { " with $it" } ?: ""),
                "settings-sync-now",
                trailing = { Icon(PhosphorIcons.Regular.ArrowsClockwise, null, Modifier.size(22.dp), tint = MaterialTheme.colorScheme.primary) },
            ) { scope.launch { runCatching { repo.syncNow() } } }

            SectionLabel("Data")
            Column {
                SettingRow("Import", "JSONL · Netscape HTML · buku database", "settings-import", trailing = {
                    Icon(PhosphorIcons.Regular.DownloadSimple, null, Modifier.size(22.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
                }) { importMenu = true }
                DropdownMenu(importMenu, { importMenu = false }, containerColor = MaterialTheme.colorScheme.surfaceContainerLow) {
                    listOf(InterchangeFormat.JSONL to "JSONL", InterchangeFormat.NETSCAPE to "Netscape HTML", InterchangeFormat.BUKU to "buku database").forEach { (f, label) ->
                        DropdownMenuItem(text = { Text(label) }, onClick = {
                            importMenu = false
                            importFormat = f
                            importer.launch(arrayOf("*/*"))
                        }, modifier = Modifier.testTag("import-${f.name.lowercase()}"))
                    }
                }
            }
            Column {
                SettingRow("Export", "JSONL · Netscape HTML", "settings-export", trailing = {
                    Icon(PhosphorIcons.Regular.UploadSimple, null, Modifier.size(22.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
                }) { exportMenu = true }
                DropdownMenu(exportMenu, { exportMenu = false }, containerColor = MaterialTheme.colorScheme.surfaceContainerLow) {
                    listOf(InterchangeFormat.JSONL to "JSONL", InterchangeFormat.NETSCAPE to "Netscape HTML").forEach { (f, label) ->
                        DropdownMenuItem(text = { Text(label) }, onClick = {
                            exportMenu = false
                            exportFormat = f
                            exporter.launch(if (f == InterchangeFormat.JSONL) "wobook.jsonl" else "wobook.html")
                        }, modifier = Modifier.testTag("export-${f.name.lowercase()}"))
                    }
                }
            }
            Text(
                "${Formatting.count(size)} bookmarks · ${Formatting.bytes(bytes)}",
                style = MaterialTheme.typography.labelMedium, color = Wb.colors.textFaint,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp).testTag("settings-size"),
            )

            SectionLabel("Behaviour")
            Text("Tapping a result", style = MaterialTheme.typography.bodyLarge, modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp))
            SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth().padding(horizontal = 16.dp).testTag("settings-tap")) {
                listOf(TapBehaviour.Detail to "Shows detail", TapBehaviour.Open to "Opens in browser").forEachIndexed { i, (tap, label) ->
                    SegmentedButton(
                        selected = state.tap == tap,
                        onClick = { scope.launch { settings.setTap(tap) } },
                        shape = SegmentedButtonDefaults.itemShape(i, 2),
                        colors = SegmentedButtonDefaults.colors(
                            activeContainerColor = MaterialTheme.colorScheme.primaryContainer,
                            activeContentColor = MaterialTheme.colorScheme.primary,
                            activeBorderColor = MaterialTheme.colorScheme.primary,
                            inactiveContainerColor = MaterialTheme.colorScheme.background,
                            inactiveBorderColor = MaterialTheme.colorScheme.outline,
                        ),
                        modifier = Modifier.testTag("tap-${tap.name.lowercase()}"),
                    ) { Text(label, style = MaterialTheme.typography.labelLarge) }
                }
            }
            SettingRow("Auto-fetch title and description", "Loads the page once when you add a URL", "settings-autofetch", trailing = {
                WbSwitch(state.autoFetch) { on -> scope.launch { settings.setAutoFetch(on) } }
            }) { scope.launch { settings.setAutoFetch(!state.autoFetch) } }

            SectionLabel("About")
            SettingRow("wobook ${BuildConfig.VERSION_NAME}", "Reinstalling loses this device's identity; peers then revoke it and you pair again.", "settings-version") {}
            SettingRow("Open-source licenses", null, "settings-licenses", trailing = {
                Icon(PhosphorIcons.Regular.CaretRight, null, Modifier.size(20.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
            }) { licenses = true }
        }
    }

    if (renaming) {
        var name by remember { mutableStateOf(deviceName) }
        AlertDialog(
            onDismissRequest = { renaming = false },
            title = { Text("Device name") },
            text = { LabeledField("Device name", name, { name = it }, imeAction = ImeAction.Done, tag = "settings-name-input") },
            confirmButton = {
                WbTextButton("Save", {
                    renaming = false
                    scope.launch {
                        runCatching { repo.setDeviceName(name) }
                            .onSuccess { deviceName = it }
                            .onFailure { snackbar.showSnackbar(it.userMessage()) }
                    }
                })
            },
            dismissButton = { WbTextButton("Cancel", { renaming = false }, color = MaterialTheme.colorScheme.onSurfaceVariant) },
            containerColor = MaterialTheme.colorScheme.surfaceContainerLow,
            shape = MaterialTheme.shapes.large,
        )
    }
    if (licenses) {
        AlertDialog(
            onDismissRequest = { licenses = false },
            title = { Text("Open-source licenses") },
            text = {
                Text(
                    "wobook — MIT\nAutomerge — MIT\nnucleo — MPL-2.0\nquinn, rustls, ring — MIT/Apache-2.0/ISC\n" +
                        "UniFFI, JNA — MPL-2.0 / Apache-2.0\nJetpack Compose, CameraX, WorkManager — Apache-2.0\n" +
                        "zxing-cpp, ZXing — Apache-2.0\nPhosphor Icons — MIT\nInter, JetBrains Mono — OFL-1.1\nCatppuccin — MIT",
                    style = MaterialTheme.typography.bodyMedium,
                )
            },
            confirmButton = { WbTextButton("Close", { licenses = false }) },
            containerColor = MaterialTheme.colorScheme.surfaceContainerLow,
            shape = MaterialTheme.shapes.large,
        )
    }
}

@Composable
private fun SettingRow(
    title: String,
    subtitle: String?,
    tag: String,
    trailing: (@Composable () -> Unit)? = null,
    onClick: () -> Unit,
) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = 64.dp).clickable(onClick = onClick).padding(horizontal = 16.dp, vertical = 10.dp).testTag(tag),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(title, style = MaterialTheme.typography.bodyLarge)
            if (subtitle != null) Text(subtitle, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        trailing?.invoke()
    }
}

@Composable
private fun WbSwitch(checked: Boolean, onChange: (Boolean) -> Unit) {
    val scheme = MaterialTheme.colorScheme
    Switch(
        checked = checked,
        onCheckedChange = onChange,
        colors = SwitchDefaults.colors(
            checkedThumbColor = scheme.onPrimary, checkedTrackColor = scheme.primary,
            uncheckedThumbColor = scheme.outline, uncheckedTrackColor = scheme.surfaceContainer, uncheckedBorderColor = scheme.outline,
        ),
    )
}

private suspend fun copyToCache(context: Context, uri: Uri, name: String): File = withContext(Dispatchers.IO) {
    val file = File(context.cacheDir, name)
    context.contentResolver.openInputStream(uri)!!.use { input -> file.outputStream().use { input.copyTo(it) } }
    file
}

